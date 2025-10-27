use crate::icon_names;

use adw::prelude::{ActionRowExt, PreferencesRowExt};
use gtk::{
    gdk, glib,
    prelude::{ButtonExt, ListBoxRowExt, WidgetExt},
};
use relm4::{
    prelude::{DynamicIndex, FactoryComponent},
    FactorySender,
};

pub struct IdentityListRow {
    pub label: String,
    pub address: String,
    identity_avatar: gtk::Image,
}

pub struct IdentityListRowInit {
    pub label: String,
    pub address: String,
}

#[derive(Debug)]
pub enum IdentityListRowOutput {
    DeleteIdentity(DynamicIndex),
    RenameIdentity(DynamicIndex),
}

#[derive(Debug)]
pub enum IdentityListRowCommand {
    LoadIdenticon(gdk::Texture),
}

#[derive(Debug)]
pub enum IdentityListRowInput {
    RenameLabel(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for IdentityListRow {
    type Init = IdentityListRowInit;
    type Input = IdentityListRowInput;
    type Output = IdentityListRowOutput;
    type CommandOutput = IdentityListRowCommand;
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        adw::ActionRow {
            set_selectable: false,
            set_activatable: false,
            #[watch]
            set_title: &self.label.to_string(),
            set_subtitle: &self.address.to_string(),
            set_subtitle_selectable: true,

            #[name(identity_avatar)]
            add_prefix = &gtk::Image {},

            add_suffix = &gtk::Button {
                set_icon_name: icon_names::EDIT,
                add_css_class: "circular",
                add_css_class: "flat",
                connect_clicked[sender, index] => move |_| {
                    let _ = sender.output(IdentityListRowOutput::RenameIdentity(index.clone()));
                },
            },
            add_suffix = &gtk::Button {
                set_icon_name: icon_names::CROSS_LARGE_CIRCLE_OUTLINE,
                add_css_class: "circular",
                add_css_class: "flat",
                connect_clicked[sender, index] => move |_| {
                    let _ = sender.output(IdentityListRowOutput::DeleteIdentity(index.clone()));
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        Self {
            label: init.label,
            address: init.address,
            identity_avatar: gtk::Image::default(),
        }
    }

    fn init_widgets(
        &mut self,
        index: &Self::Index,
        root: Self::Root,
        _returned_widget: &<Self::ParentWidget as relm4::factory::FactoryView>::ReturnedWidget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        self.identity_avatar = widgets.identity_avatar.clone();
        let address = self.address.clone();
        sender.oneshot_command(async move {
            let png_data = identicon_rs::new(address).export_png_data().unwrap();
            let texture =
                gdk::Texture::from_bytes(&glib::Bytes::from(png_data.as_slice())).unwrap();
            IdentityListRowCommand::LoadIdenticon(texture)
        });

        widgets
    }

    fn update_cmd(&mut self, message: Self::CommandOutput, _sender: FactorySender<Self>) {
        match message {
            IdentityListRowCommand::LoadIdenticon(texture) => {
                self.identity_avatar.set_paintable(Some(&texture));
            }
        }
    }

    fn update(&mut self, message: Self::Input, _sender: FactorySender<Self>) {
        match message {
            IdentityListRowInput::RenameLabel(new_label) => {
                self.label = new_label;
            }
        }
    }
}
