/*
 * A shared library that is intentionally NOT a CLAP plugin (no `clap_entry`
 * symbol). Used to verify the host rejects non-CLAP libraries cleanly.
 */
int sonix_not_a_clap_plugin(void) { return 42; }
