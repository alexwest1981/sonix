//! X11 window hosting for plugin GUIs (Fas 4.4b).
//!
//! A [`GuiSession`] ties a shared [`PluginHandle`] to a real X11 top-level
//! window. The plugin's editor is created on the main thread, reparented into
//! our window and shown; when the session is dropped the editor is hidden and
//! destroyed *before* the window goes away.
//!
//! X11 is reached through `dlopen("libX11.so.6")` (no build-time dependency on
//! `libx11-dev`) and every window operation runs on the caller's thread, which
//! must be the main thread — CLAP requires GUI calls there. The window is
//! polled from the UI frame so the plugin keeps receiving X events.
//!
//! On non-Linux targets, or builds without `plugin-host`, [`X11Window::open`]
//! returns an honest error instead of pretending to work.

use crate::audio::plugin_host_live::PluginHandle;

#[cfg(all(feature = "plugin-host", target_os = "linux"))]
mod x11 {
    use libloading::Library;
    use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_ulong, c_void};

    const EVENT_DESTROY_NOTIFY: c_int = 17;
    const EVENT_CLIENT_MESSAGE: c_int = 33;
    const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;
    const EXPOSURE_MASK: c_long = 1 << 15;

    /// `XClientMessageEvent` — the only event whose payload we inspect.
    #[repr(C)]
    struct XClientMessageEvent {
        type_: c_int,
        serial: c_ulong,
        send_event: c_int,
        display: *mut c_void,
        window: c_ulong,
        message_type: c_ulong,
        format: c_int,
        data: [c_long; 5],
    }

    /// Backing store for `XEvent`; 256 bytes is safely above `sizeof(XEvent)`
    /// on every Xlib ABI we target (192 bytes on LP64).
    #[repr(C, align(8))]
    struct XEvent([u8; 256]);

    struct X11Api {
        open_display: unsafe extern "C" fn(*const c_char) -> *mut c_void,
        close_display: unsafe extern "C" fn(*mut c_void) -> c_int,
        default_screen: unsafe extern "C" fn(*mut c_void) -> c_int,
        root_window: unsafe extern "C" fn(*mut c_void, c_int) -> c_ulong,
        black_pixel: unsafe extern "C" fn(*mut c_void, c_int) -> c_ulong,
        create_simple_window: unsafe extern "C" fn(
            *mut c_void,
            c_ulong,
            c_int,
            c_int,
            c_uint,
            c_uint,
            c_uint,
            c_ulong,
            c_ulong,
        ) -> c_ulong,
        store_name: unsafe extern "C" fn(*mut c_void, c_ulong, *const c_char) -> c_int,
        select_input: unsafe extern "C" fn(*mut c_void, c_ulong, c_long) -> c_int,
        map_window: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
        destroy_window: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
        resize_window: unsafe extern "C" fn(*mut c_void, c_ulong, c_uint, c_uint) -> c_int,
        flush: unsafe extern "C" fn(*mut c_void) -> c_int,
        pending: unsafe extern "C" fn(*mut c_void) -> c_int,
        next_event: unsafe extern "C" fn(*mut c_void, *mut XEvent) -> c_int,
        intern_atom: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_ulong,
        set_wm_protocols: unsafe extern "C" fn(*mut c_void, c_ulong, *mut c_ulong, c_int) -> c_int,
    }

    unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
        unsafe { lib.get::<T>(name).map(|s| *s).map_err(|e| e.to_string()) }
    }

    impl X11Api {
        fn load() -> Result<(Library, Self), String> {
            let mut loaded = None;
            for name in ["libX11.so.6", "libX11.so"] {
                if let Ok(lib) = unsafe { Library::new(name) } {
                    loaded = Some(lib);
                    break;
                }
            }
            let lib = loaded
                .ok_or_else(|| "Kunde inte ladda libX11 (installera libx11-6)".to_string())?;
            let api = unsafe {
                Self {
                    open_display: sym(&lib, b"XOpenDisplay\0")?,
                    close_display: sym(&lib, b"XCloseDisplay\0")?,
                    default_screen: sym(&lib, b"XDefaultScreen\0")?,
                    root_window: sym(&lib, b"XRootWindow\0")?,
                    black_pixel: sym(&lib, b"XBlackPixel\0")?,
                    create_simple_window: sym(&lib, b"XCreateSimpleWindow\0")?,
                    store_name: sym(&lib, b"XStoreName\0")?,
                    select_input: sym(&lib, b"XSelectInput\0")?,
                    map_window: sym(&lib, b"XMapWindow\0")?,
                    destroy_window: sym(&lib, b"XDestroyWindow\0")?,
                    resize_window: sym(&lib, b"XResizeWindow\0")?,
                    flush: sym(&lib, b"XFlush\0")?,
                    pending: sym(&lib, b"XPending\0")?,
                    next_event: sym(&lib, b"XNextEvent\0")?,
                    intern_atom: sym(&lib, b"XInternAtom\0")?,
                    set_wm_protocols: sym(&lib, b"XSetWMProtocols\0")?,
                }
            };
            Ok((lib, api))
        }
    }

    /// A host-owned X11 top-level window that a plugin editor embeds into.
    pub struct X11Window {
        /// Kept alive so the resolved symbols stay valid.
        _lib: Library,
        api: X11Api,
        display: *mut c_void,
        window: c_ulong,
        wm_delete: c_ulong,
        alive: bool,
    }

    // The window is only ever touched from the thread that created it (the
    // main thread); `GuiSession` enforces that by living in the UI.
    unsafe impl Send for X11Window {}

    impl X11Window {
        pub fn open(title: &str, width: u32, height: u32) -> Result<Self, String> {
            let (lib, api) = X11Api::load()?;
            let display = unsafe { (api.open_display)(std::ptr::null()) };
            if display.is_null() {
                return Err(
                    "Ingen X-display tillgänglig (sätt DISPLAY för att visa plugin-GUI:t)".to_string(),
                );
            }
            let screen = unsafe { (api.default_screen)(display) };
            let root = unsafe { (api.root_window)(display, screen) };
            let black = unsafe { (api.black_pixel)(display, screen) };
            let width = width.clamp(64, 8192);
            let height = height.clamp(64, 8192);
            let window = unsafe {
                (api.create_simple_window)(display, root, 0, 0, width, height, 0, black, black)
            };
            if window == 0 {
                unsafe { (api.close_display)(display) };
                return Err("Kunde inte skapa X11-fönstret".to_string());
            }

            let title = CString::new(title).unwrap_or_else(|_| c"Plugin".to_owned());
            unsafe { (api.store_name)(display, window, title.as_ptr()) };
            unsafe { (api.select_input)(display, window, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK) };

            let wm_delete = unsafe { (api.intern_atom)(display, c"WM_DELETE_WINDOW".as_ptr(), 0) };
            if wm_delete != 0 {
                let mut protocols = [wm_delete];
                unsafe { (api.set_wm_protocols)(display, window, protocols.as_mut_ptr(), 1) };
            }

            unsafe { (api.map_window)(display, window) };
            unsafe { (api.flush)(display) };

            Ok(Self {
                _lib: lib,
                api,
                display,
                window,
                wm_delete,
                alive: true,
            })
        }

        pub fn window_id(&self) -> u64 {
            self.window as u64
        }

        pub fn resize(&self, width: u32, height: u32) {
            if !self.alive {
                return;
            }
            unsafe {
                (self.api.resize_window)(self.display, self.window, width.clamp(64, 8192), height.clamp(64, 8192));
                (self.api.flush)(self.display);
            }
        }

        /// Drains pending X events. Returns `false` once the window is gone.
        pub fn poll(&mut self) -> bool {
            if !self.alive {
                return false;
            }
            unsafe {
                while (self.api.pending)(self.display) > 0 {
                    let mut event = XEvent([0u8; 256]);
                    (self.api.next_event)(self.display, &mut event);
                    let type_ = *(event.0.as_ptr() as *const c_int);
                    if type_ == EVENT_DESTROY_NOTIFY {
                        self.alive = false;
                        return false;
                    }
                    if type_ == EVENT_CLIENT_MESSAGE && self.wm_delete != 0 {
                        let msg = &*(event.0.as_ptr() as *const XClientMessageEvent);
                        if msg.message_type == self.wm_delete
                            && msg.data[0] as c_ulong == self.wm_delete
                        {
                            self.alive = false;
                            return false;
                        }
                    }
                }
            }
            true
        }

        pub fn is_alive(&self) -> bool {
            self.alive
        }
    }

    impl Drop for X11Window {
        fn drop(&mut self) {
            if self.display.is_null() {
                return;
            }
            unsafe {
                if self.alive {
                    (self.api.destroy_window)(self.display, self.window);
                }
                (self.api.flush)(self.display);
                (self.api.close_display)(self.display);
            }
        }
    }
}

#[cfg(not(all(feature = "plugin-host", target_os = "linux")))]
mod x11 {
    /// Honest stub: window hosting needs Linux + the `plugin-host` feature.
    pub struct X11Window;

    impl X11Window {
        pub fn open(_title: &str, _width: u32, _height: u32) -> Result<Self, String> {
            Err("X11-fönsterhosting kräver Linux och --features plugin-host".to_string())
        }
        pub fn window_id(&self) -> u64 {
            0
        }
        pub fn resize(&self, _width: u32, _height: u32) {}
        pub fn poll(&mut self) -> bool {
            false
        }
        pub fn is_alive(&self) -> bool {
            false
        }
    }
}

pub use x11::X11Window;

/// An open plugin editor embedded in a host window.
///
/// Dropping the session hides and destroys the plugin GUI, then tears the
/// window down — the order CLAP requires.
pub struct GuiSession {
    handle: PluginHandle,
    window: X11Window,
    title: String,
}

impl GuiSession {
    /// Creates the plugin editor and embeds it into a fresh X11 window. All
    /// calls happen on the current thread, which must be the main thread.
    pub fn open(handle: PluginHandle, title: &str) -> Result<Self, String> {
        if !handle.gui_is_api_supported("x11", false) {
            return Err(crate::i18n::t("Pluginen stöder inte inbäddat X11-GUI").to_string());
        }
        let (width, height) = handle.gui_get_size().unwrap_or((640, 480));
        if !handle.gui_create("x11", false) {
            return Err(crate::i18n::t("Kunde inte skapa plugin-GUI:t").to_string());
        }
        let window = match X11Window::open(title, width, height) {
            Ok(window) => window,
            Err(err) => {
                handle.gui_destroy();
                return Err(err);
            }
        };
        if !handle.gui_set_parent(window.window_id()) {
            handle.gui_destroy();
            return Err(crate::i18n::t("Kunde inte bädda in plugin-GUI:t").to_string());
        }
        if !handle.gui_show() {
            handle.gui_destroy();
            return Err(crate::i18n::t("Kunde inte visa plugin-GUI:t").to_string());
        }
        Ok(Self {
            handle,
            window,
            title: title.to_string(),
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn is_alive(&self) -> bool {
        self.window.is_alive()
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        self.handle.gui_get_size()
    }

    /// Resizes the host window and tells the plugin editor about the new size.
    /// Currently unused by the UI, but part of the host GUI surface.
    #[allow(dead_code)]
    pub fn resize(&self, width: u32, height: u32) {
        self.window.resize(width, height);
        self.handle.gui_set_size(width, height);
    }

    /// Drains window events. Returns `false` when the user closed the window,
    /// so the caller can drop the session.
    pub fn poll(&mut self) -> bool {
        self.window.poll()
    }
}

impl Drop for GuiSession {
    fn drop(&mut self) {
        self.handle.gui_hide();
        self.handle.gui_destroy();
        // The X11 window is torn down after the editor, as CLAP requires.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::plugin_host_live::{PluginCore, PluginInfo};
    use std::sync::Arc;

    struct FakeCore {
        x11: bool,
        created: std::sync::atomic::AtomicBool,
    }

    impl PluginCore for FakeCore {
        fn info(&self) -> &PluginInfo {
            static INFO: std::sync::OnceLock<PluginInfo> = std::sync::OnceLock::new();
            INFO.get_or_init(|| PluginInfo {
                id: "fake".into(),
                name: "Fake".into(),
                ..Default::default()
            })
        }
        fn latency_frames(&self) -> u32 {
            0
        }
        fn save_state(&self) -> Vec<u8> {
            Vec::new()
        }
        fn load_state(&self, _data: &[u8]) -> bool {
            false
        }
        fn set_parameter(&self, _id: u32, _value: f64) -> bool {
            true
        }
        fn reset(&self) {}
        fn preset_load(&self, _location: &str) -> bool {
            false
        }
        fn gui_is_api_supported(&self, api: &str, _is_floating: bool) -> bool {
            self.x11 && api == "x11"
        }
        fn gui_preferred_api(&self) -> Option<(String, bool)> {
            Some(("x11".to_string(), false))
        }
        fn gui_get_size(&self) -> Option<(u32, u32)> {
            Some((320, 240))
        }
        fn gui_can_resize(&self) -> bool {
            true
        }
        fn gui_is_created(&self) -> bool {
            self.created.load(std::sync::atomic::Ordering::SeqCst)
        }
        fn gui_create(&self, _api: &str, _is_floating: bool) -> bool {
            self.created.store(true, std::sync::atomic::Ordering::SeqCst);
            true
        }
        fn gui_set_parent(&self, _x11_window: u64) -> bool {
            true
        }
        fn gui_set_size(&self, _width: u32, _height: u32) -> bool {
            true
        }
        fn gui_show(&self) -> bool {
            true
        }
        fn gui_hide(&self) -> bool {
            true
        }
        fn gui_destroy(&self) {
            self.created.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn handle(x11: bool) -> PluginHandle {
        PluginHandle::new(Arc::new(FakeCore {
            x11,
            created: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    #[test]
    fn rejects_plugins_without_x11_support() {
        let err = match GuiSession::open(handle(false), "Nope") {
            Ok(_) => panic!("a plugin without clap.gui x11 must be rejected"),
            Err(e) => e,
        };
        assert!(err.contains("X11"));
    }

    #[test]
    fn x11_open_without_display_is_an_honest_error() {
        // In a headless environment there is no DISPLAY, so opening a window
        // must fail cleanly and destroy the editor it created.
        if std::env::var_os("DISPLAY").is_some() {
            return; // a real display exists; nothing to assert headlessly
        }
        let err = match X11Window::open("Headless", 320, 240) {
            Ok(_) => panic!("expected an error without DISPLAY"),
            Err(e) => e,
        };
        assert!(!err.is_empty());
    }

    #[cfg(all(feature = "plugin-host", target_os = "linux"))]
    #[test]
    fn mock_plugin_reaches_the_x11_stage_without_display() {
        if std::env::var_os("DISPLAY").is_some() {
            return; // a real display would let the window actually open
        }
        let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
            return;
        };
        let processor =
            crate::audio::plugin_host_live::load_processor(mock, 48_000.0, 128).expect("mock");
        let insert = crate::audio::plugin_host_live::PluginInsert::new(processor, 128);
        let handle = insert.core_handle().expect("handle");
        assert!(handle.gui_is_api_supported("x11", false));
        let err = match GuiSession::open(handle, "Mock – Sonix") {
            Ok(_) => panic!("headless environment must not open a window"),
            Err(e) => e,
        };
        assert!(err.contains("X-display"), "unexpected error: {err}");
    }
}
