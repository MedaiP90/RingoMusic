---
name: gtk-apps
description: Rules for building GTK 4 desktop apps for Linux and GNOME, mainly in Rust with gtk4-rs and libadwaita, with notes for C. Use this skill every time Claude writes or changes GTK code, .ui or .blp files, GResource or GSettings XML, meson.build, desktop or metainfo files, or a Flatpak manifest, or when the user asks about GTK widgets, GObject subclasses, signals, properties, actions, list views, or GNOME app packaging. Use it together with the rust-best-practices skill for Rust code.
---

# GTK 4 App Best Practices

## Scope

This skill gives rules for GTK 4 apps. It is based on these sources:

- GTK 4 "Getting Started": https://docs.gtk.org/gtk4/getting_started.html
- GTK 4 Tutorial for Beginners (Toshio Sekiya): https://toshiocp.github.io/Gtk4-tutorial/
- GNOME Project Handbook: https://handbook.gnome.org/
- The gtk4-rs book and the libadwaita documentation, for the Rust API names.

The default language is Rust with `gtk4` (imported as `gtk`), `libadwaita` (imported as `adw`), `glib`, and `gio`. Section 14 gives the C equivalents.

This skill gives only GTK-specific rules. For general Rust rules, use the `rust-best-practices` skill. For naming, function size, and SOLID rules, use the `clean-code` and `oop-principles` skills.

Write all code, identifiers, comments, and untranslated UI source strings in English.

## 1. Rules that you must not break

1. **Do not block the main thread.** GTK draws and handles input on the main thread. A blocking call freezes the window. Section 9 shows how to do slow work.
2. **Touch widgets only from the main thread.** GTK objects are not `Send`. Send data to the main thread. Do not send widgets to other threads.
3. **Do not make reference cycles in signal handlers.** A closure that holds a strong reference to its own widget causes a memory leak. Use weak references (Section 6).
4. **Do not use deprecated widgets in new code.** Section 10 lists the replacements.
5. **Keep the layout in UI files, not in code.** Use composite templates (Section 4).

## 2. Application structure

### 2.1 Application ID

- Use a reverse-DNS application ID that you control: `io.github.username.AppName`. Use the format from the GTK documentation for application IDs.
- Use the same ID for these items:
  - The `GtkApplication` or `AdwApplication`.
  - The GSettings schema ID.
  - The desktop file name: `<APP_ID>.desktop`.
  - The metainfo file name: `<APP_ID>.metainfo.xml`.
  - The icon name.
  - The GResource base path: `/io/github/username/AppName/`.
- For development builds, add the suffix `.Devel` to the ID. Then the development build and the installed build can run at the same time.

### 2.2 Entry point

- Create one `adw::Application` (or `gtk::Application` if you do not use libadwaita).
- Connect `startup` to register actions, keyboard shortcuts, and CSS. `startup` runs one time in the primary instance.
- Connect `activate` to create and present the main window. `activate` can run more than one time. If a window exists, present it again. Do not create a second window.
- To open files from the command line, set `gio::ApplicationFlags::HANDLES_OPEN` and connect `open`.
- Return the exit code from `app.run()`.

```rust
use adw::prelude::*;
use gtk::{gio, glib};

const APP_ID: &str = "io.github.username.AppName";

fn main() -> glib::ExitCode {
    gio::resources_register_include!("app.gresource")
        .expect("resources are compiled into the binary by build.rs");

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(setup_actions_and_shortcuts);
    app.connect_activate(present_main_window);
    app.run()
}

fn present_main_window(app: &adw::Application) {
    let window = app
        .active_window()
        .unwrap_or_else(|| Window::new(app).upcast());
    window.present();
}
```

### 2.3 Recommended project layout

```
Cargo.toml
build.rs                 # compiles the GResource bundle
meson.build              # optional: install data, schemas, translations
data/
  <APP_ID>.desktop.in
  <APP_ID>.metainfo.xml.in
  <APP_ID>.gschema.xml
  icons/
resources/
  resources.gresource.xml
  ui/window.ui           # or window.blp with Blueprint
  style.css
po/
  POTFILES.in
  LINGUAS
src/
  main.rs
  application.rs
  window/
    mod.rs               # public wrapper type
    imp.rs               # private implementation
build-aux/
  <APP_ID>.Devel.json    # Flatpak manifest
```

Give each custom GObject type its own module with two files: `mod.rs` for the public wrapper and `imp.rs` for the implementation struct.

## 3. GObject subclasses

Use subclasses for windows, custom widgets, and model items. Follow this pattern.

```rust
// src/window/imp.rs
use std::cell::RefCell;

use adw::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};

#[derive(CompositeTemplate, Debug, Default)]
#[template(resource = "/io/github/username/AppName/ui/window.ui")]
pub struct Window {
    #[template_child]
    pub save_button: TemplateChild<gtk::Button>,
    pub current_path: RefCell<Option<std::path::PathBuf>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Window {
    const NAME: &'static str = "AppNameWindow";
    type Type = super::Window;
    type ParentType = adw::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
        klass.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

#[gtk::template_callbacks]
impl Window {
    #[template_callback]
    fn on_save_clicked(&self, _button: &gtk::Button) {
        self.obj().save();
    }
}

impl ObjectImpl for Window {
    fn constructed(&self) {
        self.parent_constructed();
        // Set up things that need the finished object here.
    }
}

impl WidgetImpl for Window {}
impl WindowImpl for Window {}
impl ApplicationWindowImpl for Window {}
impl AdwApplicationWindowImpl for Window {}
```

```rust
// src/window/mod.rs
mod imp;

use adw::subclass::prelude::*;
use gtk::{gio, glib};

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn save(&self) {
        // Public behavior goes on the wrapper type.
    }
}
```

Rules:

- Give the type `NAME` a prefix from the app name (`AppNameWindow`). Two GObject types with the same name in one process cause a crash.
- Put state in the `imp` struct. Use `Cell<T>` for `Copy` values and `RefCell<T>` for other values. Use `OnceCell<T>` for values that you set one time in `constructed`.
- Do not hold a `RefCell` borrow across a call that can emit a signal. A signal handler can try to borrow the same cell, and the program panics.
- Put public methods on the wrapper type (`mod.rs`). Get the implementation with `self.imp()`. In the implementation, get the wrapper with `self.obj()`.
- Override `dispose` to drop resources that hold references to other objects. In Rust, `Drop` of the `imp` fields does most of the cleanup.

## 4. UI files and composite templates

- Describe each window, dialog, and complex widget in a UI file. Bind it to a subclass with `#[template]`.
- Use Blueprint (`.blp`) if the project already uses it. Otherwise, use GtkBuilder XML (`.ui`). Do not mix the two formats in one project.
- Bind widgets that the code needs with `#[template_child]`. Do not use `gtk::Builder::object()` with string IDs in templates.
- Connect signals in the UI file with `<signal name="clicked" handler="on_save_clicked" swapped="true"/>` and `#[template_callback]`. Do not use `connect_*` in code for signals that the UI file can declare.
- Mark each user-visible string in a UI file as translatable: `<property name="label" translatable="yes">Save</property>`.
- Use code for the UI only when the widgets change at runtime (for example, rows that come from data). Even then, prefer a list view with a model (Section 8).

```xml
<?xml version="1.0" encoding="UTF-8"?>
<interface>
  <template class="AppNameWindow" parent="AdwApplicationWindow">
    <property name="title" translatable="yes">App Name</property>
    <property name="default-width">800</property>
    <property name="default-height">600</property>
    <property name="content">
      <object class="AdwToolbarView">
        <child type="top">
          <object class="AdwHeaderBar"/>
        </child>
        <property name="content">
          <object class="GtkButton" id="save_button">
            <property name="label" translatable="yes">_Save</property>
            <property name="use-underline">true</property>
            <signal name="clicked" handler="on_save_clicked" swapped="true"/>
          </object>
        </property>
      </object>
    </property>
  </template>
</interface>
```

## 5. Resources, CSS, and settings

### 5.1 GResource

- Put UI files, CSS, and icons that the app uses at runtime in a GResource bundle. Then you do not install these files separately.
- Compile the bundle in `build.rs` with `glib-build-tools`. Register it at the start of `main`.

```rust
// build.rs
fn main() {
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "app.gresource",
    );
}
```

- If you use Meson, use `gnome.compile_resources()` instead.
- `AdwApplication` loads `style.css`, `style-dark.css`, and `icons/` automatically from the resource base path of the app. Put the files there. Do not load them by hand.

### 5.2 CSS

- Use CSS for visual style only. Do not use CSS to fix the layout.
- Add and remove style classes with `add_css_class()` and `remove_css_class()`. Do not use `GtkStyleContext`. It is deprecated.
- Use the style classes and named colors of libadwaita (`suggested-action`, `destructive-action`, `card`, `boxed-list`, `var(--accent-bg-color)`). Do not hard-code colors. Hard-coded colors break the dark style and the high-contrast style.

### 5.3 GSettings

- Store user preferences and window state in GSettings. Do not write your own config file for these values.
- Use the app ID as the schema ID and `/io/github/username/AppName/` as the path.
- Bind settings to widget properties with `settings.bind(key, &widget, property).build()`.
- Save the window size and the maximized state in `close_request`. Restore them in `constructed`.
- Expose a boolean key as a menu toggle with `settings.create_action(key)`.
- Validate the schema with `glib-compile-schemas --strict`.

## 6. Signals, closures, and memory

- Use weak references in closures that capture a widget or an object. Use the `glib::clone!` macro.

```rust
self.save_button.connect_clicked(glib::clone!(
    #[weak(rename_to = window)]
    self.obj(),
    move |_| window.save()
));
```

- Use `#[strong]` only when the closure must keep the object alive, and the object does not own the closure.
- Use `#[upgrade_or]` or `#[upgrade_or_default]` when the closure must return a value.
- Disconnect a handler with its `SignalHandlerId` when the source object lives longer than the receiver.
- Define custom signals in `ObjectImpl::signals()` with a `static OnceLock<Vec<Signal>>`. Give signals kebab-case names: `item-removed`.
- Prefer a property and `notify` to a custom signal when the event is "a value changed".

## 7. Properties and actions

### 7.1 Properties

- Declare properties with `#[derive(glib::Properties)]` and `#[glib::derived_properties]`. Do not write `properties()`, `set_property()`, and `property()` by hand.

```rust
#[derive(Default, glib::Properties)]
#[properties(wrapper_type = super::Task)]
pub struct Task {
    #[property(get, set)]
    title: RefCell<String>,
    #[property(get, set)]
    done: Cell<bool>,
}

#[glib::derived_properties]
impl ObjectImpl for Task {}
```

- Give properties kebab-case names. The macro converts `snake_case` field names.
- Connect two properties with `bind_property(...).sync_create().build()`. Do not copy values by hand in two `notify` handlers.
- Use `gtk::Expression` (for example, `property_expression`) to bind a property of a list item to a widget in a list row.

### 7.2 Actions, menus, and shortcuts

- Do every user command through a `gio::Action`. Buttons, menu items, and keyboard shortcuts then call the same code. GTK disables all of them when the action is disabled.
- Register app-wide actions (`quit`, `about`, `preferences`) on the application with the prefix `app.`. Register window actions with the prefix `win.`.
- Use `gio::ActionEntry` and `add_action_entries()`.

```rust
let close = gio::ActionEntry::builder("close")
    .activate(|window: &Window, _, _| window.close())
    .build();
window.add_action_entries([close]);
app.set_accels_for_action("win.close", &["<Ctrl>w"]);
```

- Use stateful actions for toggles and radio choices. Use `gio::PropertyAction` to make a property into an action.
- Define menus in UI files with `<menu>`, `<section>`, and `<item>`. Attach them to a `GtkMenuButton` in the header bar. Use the primary menu icon `open-menu-symbolic`.
- Disable an action with `action.set_enabled(false)` when the command is not possible. Do not hide the button.

## 8. Lists and data

- Use `GListModel` (`gio::ListStore`) with `GtkListView`, `GtkGridView`, or `GtkColumnView`. These views create rows only for visible items, so they work with large lists.
- Use `GtkListBox` only for short, static lists, for example a preferences page with a `boxed-list` style.
- Put the data in a model item type (a GObject subclass with properties). Do not store data in widgets.
- Create rows with `SignalListItemFactory`:
  - `setup`: create the row widget one time.
  - `bind`: connect the row widget to the item. Use property bindings or expressions.
  - `unbind`: remove the bindings that `bind` created.
- Sort and filter with `GtkSortListModel` and `GtkFilterListModel`. Do not sort the store by hand.
- Use `GtkSingleSelection`, `GtkMultiSelection`, or `GtkNoSelection` as the model of the view.
- Use `GtkDropDown` for a choice from a list.

## 9. Async and slow work

- Run async code on the main loop with `glib::spawn_future_local`. Await GIO async functions (`file.load_contents_future()`, `FileDialog::open_future()`).
- Run blocking or CPU-heavy work with `gio::spawn_blocking`, then await the result on the main thread.

```rust
glib::spawn_future_local(glib::clone!(
    #[weak(rename_to = window)]
    self,
    async move {
        let result = gio::spawn_blocking(move || parse_large_file(&path)).await;
        match result {
            Ok(Ok(document)) => window.show_document(&document),
            Ok(Err(error)) => window.show_error_toast(&error.to_string()),
            Err(_) => window.show_error_toast("internal error: parser thread panicked"),
        }
    }
));
```

- If you use Tokio for network code, run the Tokio runtime on its own threads. Send the results to the main thread with an `async-channel`. Receive them in a `glib::spawn_future_local` task.
- Show progress for work that takes more than about one second. Use a spinner, a progress bar, or a toast.
- Show errors to the user with `adw::Toast`, `adw::Banner`, or `adw::AlertDialog`. Do not show raw debug output.

## 10. Deprecated APIs and replacements

Do not use the APIs in the left column in new code. GTK 4.10 and later deprecated most of them.

| Do not use | Use |
|---|---|
| `GtkTreeView`, `GtkCellRenderer*`, `GtkIconView` | `GtkListView`, `GtkColumnView`, `GtkGridView` |
| `GtkComboBox`, `GtkComboBoxText` | `GtkDropDown` |
| `GtkEntryCompletion` | A `GtkPopover` with a list view |
| `GtkDialog`, `GtkMessageDialog` | `adw::AlertDialog`, `adw::Dialog`, or `gtk::AlertDialog` |
| `GtkFileChooserDialog`, `GtkFileChooserNative` | `gtk::FileDialog` |
| `GtkColorChooserDialog`, `GtkColorButton` | `gtk::ColorDialog`, `gtk::ColorDialogButton` |
| `GtkFontChooserDialog`, `GtkFontButton` | `gtk::FontDialog`, `gtk::FontDialogButton` |
| `GtkStyleContext` methods | `add_css_class()`, `remove_css_class()`, `color()` |
| `GtkInfoBar` | `adw::Banner` |
| `GtkStatusbar` | `adw::Toast` or a label |
| `gtk_widget_show()` on a window | `present()` |
| `gtk-application-prefer-dark-theme` | `adw::StyleManager` |
| `GtkShortcutsWindow` | `adw::ShortcutsDialog` (libadwaita 1.8 and later) |

Before you use an API, check its documentation for a "Deprecated since" note.

## 11. Layout and GNOME design

- Follow the GNOME Human Interface Guidelines for GNOME apps.
- Use libadwaita widgets for the window structure:
  - `AdwApplicationWindow` with `AdwToolbarView` and `AdwHeaderBar`.
  - `AdwNavigationView` or `AdwNavigationSplitView` for navigation.
  - `AdwViewStack` with `AdwViewSwitcher` for top-level views.
  - `AdwPreferencesDialog` for preferences. `AdwAboutDialog` for the about window.
  - `AdwToastOverlay` for short messages.
- Use `AdwBreakpoint` to change the layout on narrow windows. The app must work at 360 px width if it targets mobile devices.
- Use `GtkBox` and `GtkGrid` for simple layouts. Control the space with `halign`, `valign`, `hexpand`, `vexpand`, and `spacing`. Do not use `GtkFixed` for normal layouts.
- Use symbolic icons (`*-symbolic`) in header bars and buttons.
- Use `GtkDrawingArea` with `set_draw_func()` for custom drawing. Call `queue_draw()` to redraw. For a custom widget, override `snapshot()`.
- Handle input with event controllers (`GestureClick`, `GestureDrag`, `EventControllerKey`). Add them with `add_controller()`.

## 12. Accessibility and translation

- Give every icon-only button a tooltip and an accessible label. The tooltip text is the accessible label by default.
- Use mnemonics (`_Save` with `use-underline`) on labels and buttons in dialogs.
- Make all features available with the keyboard. Test with the Tab key and the arrow keys.
- Connect a label to its widget with the `mnemonic-widget` property or with an accessible relation.
- Wrap every user-visible string in code with `gettext("...")` (the `gettext-rs` crate). Mark strings in UI files with `translatable="yes"`.
- Do not build sentences from parts. Use one format string with placeholders, so that translators can change the word order.
- List every file with translatable strings in `po/POTFILES.in`.

## 13. Build, packaging, and contribution

### 13.1 Install the data files

A GNOME app installs these files. Meson installs them in the correct directories.

| File | Directory |
|---|---|
| Binary | `bin/` |
| `<APP_ID>.desktop` | `share/applications/` |
| `<APP_ID>.metainfo.xml` | `share/metainfo/` |
| `<APP_ID>.svg` | `share/icons/hicolor/scalable/apps/` |
| `<APP_ID>-symbolic.svg` | `share/icons/hicolor/symbolic/apps/` |
| `<APP_ID>.gschema.xml` | `share/glib-2.0/schemas/` |

Validate the files in CI:

```sh
desktop-file-validate data/<APP_ID>.desktop
appstreamcli validate --no-net data/<APP_ID>.metainfo.xml
glib-compile-schemas --strict --dry-run data/
```

### 13.2 Build and distribution

- Use Meson as the top-level build system for apps with data files. Meson calls Cargo for the Rust code.
- Use Flatpak with the GNOME runtime as the main distribution format. Add the `rust-stable` SDK extension. Generate offline Cargo sources with `flatpak-cargo-generator`.
- Use GNOME Builder or a Flatpak build for app development. Use Toolbx when the app needs host resources that the Flatpak sandbox blocks.
- Set `G_MESSAGES_DEBUG=all` and `GTK_DEBUG=interactive` to debug. The GTK Inspector (`Ctrl+Shift+D`) shows the widget tree and CSS.

### 13.3 Commit messages (GNOME Handbook)

1. Write a summary line of 72 characters or fewer. Use the imperative mood: "Add search to sidebar". Do not end it with a period. You can start with one tag for the part of the module: `sidebar: Add search`.
2. Add a blank line.
3. Write a body that explains what changed and why. Do not explain how; the diff shows that. Keep lines to 75 characters or fewer.
4. Add references at the end: `Closes #123` or `Fixes #123`.
5. For a revert, explain why you reverted the commit.

## 14. Notes for C code

Use these rules when the project uses C:

- Include only `<gtk/gtk.h>`. Do not include other GTK headers.
- Declare types with `G_DECLARE_FINAL_TYPE` and define them with `G_DEFINE_FINAL_TYPE`. Put the parent instance as the first member of the struct.
- Bind templates in `class_init` with `gtk_widget_class_set_template_from_resource()`, `gtk_widget_class_bind_template_child()`, and `gtk_widget_class_bind_template_callback()`. Call `gtk_widget_init_template()` in `init`.
- Release references in `dispose` with `g_clear_object()` and `g_clear_pointer()`. Free other memory in `finalize`. Always chain up to the parent class.
- Use `g_autoptr()` and `g_autofree` for local values.
- Use the type-checked cast macros (`GTK_WINDOW (obj)`). They print a warning when the type is wrong.
- Use `g_signal_connect_swapped()` when the callback must get the user data as its first argument.
- Report errors with `GError **error` parameters. Do not ignore a returned `GError`.
- Build with Meson and `dependency('gtk4')` and `dependency('libadwaita-1')`.

## 15. Review checklist

Before you finish a GTK change, make sure that each item is true:

- [ ] No blocking call runs on the main thread.
- [ ] No widget is used from another thread.
- [ ] Closures that capture objects use `#[weak]` references.
- [ ] Layouts are in UI files with composite templates.
- [ ] UI files, CSS, and icons are in the GResource bundle.
- [ ] User commands are actions with `app.` or `win.` prefixes and keyboard shortcuts.
- [ ] Lists use a `GListModel` with a list view and a factory.
- [ ] No deprecated API from Section 10 is used.
- [ ] Every user-visible string is translatable.
- [ ] Every icon-only button has a tooltip.
- [ ] The app ID is the same in all data files, and the validators pass.
