fn main() {
    relm4_icons_build::bundle_icons(
        // Name of the file that will be generated at `OUT_DIR`
        "icon_names.rs",
        // Optional app ID
        Some("net.pigeoncatcher.ripple"),
        // Custom base resource path:
        // * defaults to `/com/example/myapp` in this case if not specified explicitly
        // * or `/org/relm4` if app ID was not specified either
        None::<&str>,
        // Directory with custom icons (if any)
        None::<&str>,
        // List of icons to include
        [
            "person",
            "mail-inbox-filled",
            "desktop-pulse-filled",
            "plus",
            "edit",
            "cross-large-circle-outline",
        ],
    );
}
