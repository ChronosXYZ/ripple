use relm4::component::{AsyncComponent, AsyncComponentController, AsyncController};
use relm4::gtk::prelude::*;
use relm4::{
    adw, gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    SimpleComponent,
};
use relm4_icons::icon_name;

use crate::components::identities_list::IdentitiesListInput;

use super::components::dialogs::identity_dialog::{IdentityDialogModel, IdentityDialogOutput};
use super::components::identities_list::{IdentitiesListModel, IdentitiesListOutput};
use super::components::message_composer::MessageComposer;
use super::components::messages::{MessagesInput, MessagesModel};
use super::components::network_status::NetworkStatusModel;

pub(crate) struct AppModel {
    identities_list: AsyncController<IdentitiesListModel>,
    messages: AsyncController<MessagesModel>,
    network_status: AsyncController<NetworkStatusModel>,
    stack: adw::ViewStack,
    show_plus_button: bool,
    identity_dialog: Controller<IdentityDialogModel>,
}

#[derive(Debug)]
pub(crate) enum AppInput {
    PageChanged,
    HandleClickPlusButton,
    ShowPlusButton(bool),
    IdentitiesListUpdated,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Input = AppInput;
    type Output = ();
    type Init = ();

    view! {
        adw::ApplicationWindow {
            set_default_size: (800, 600),

            set_title = Some("Ripple"),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    set_centering_policy: adw::CenteringPolicy::Strict,

                    #[wrap(Some)]
                    #[name="view_title"]
                    set_title_widget = &adw::ViewSwitcherTitle {
                        set_stack: Some(&stack),
                        set_title: "Ripple"
                    },
                    pack_start = if model.show_plus_button {
                        gtk::Button{
                            set_icon_name: icon_name::PLUS,
                            connect_clicked => AppInput::HandleClickPlusButton
                        }
                    } else { gtk::Box{} }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_vexpand: true,

                    #[name="stack"]
                    adw::ViewStack {
                        set_vexpand: true,

                        connect_visible_child_name_notify => AppInput::PageChanged,

                        add_titled[Some("identities"), "Identities"] = model.identities_list.widget() -> &gtk::ScrolledWindow{} -> {
                            set_icon_name: Some(icon_name::PERSON),
                        },

                        add_titled[Some("messages"), "Messages"] = model.messages.widget() -> &gtk::ScrolledWindow {} -> {
                            set_icon_name: Some(icon_name::MAIL_INBOX_FILLED),
                        },

                        add_titled[Some("status"), "Network Status"] = model.network_status.widget() -> &gtk::ScrolledWindow {} -> {
                            set_icon_name: Some(icon_name::DESKTOP_PULSE_FILLED),
                        },
                    },

                    #[name = "view_bar"]
                    adw::ViewSwitcherBar {
                        set_stack: Some(&stack),
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: &Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let identities_list_component =
            IdentitiesListModel::builder()
                .launch(())
                .forward(sender.input_sender(), |message| match message {
                    IdentitiesListOutput::EmptyList(v) => AppInput::ShowPlusButton(!v),
                    IdentitiesListOutput::IdentitiesListUpdated => AppInput::IdentitiesListUpdated,
                });
        let messages_component = MessagesModel::builder().launch(()).detach();
        let network_status_component = NetworkStatusModel::builder().launch(()).detach();

        let identity_dialog_controller = IdentityDialogModel::builder().launch(None).forward(
            identities_list_component.sender(),
            |message| match message {
                IdentityDialogOutput::GenerateIdentity(label) => {
                    IdentitiesListInput::GenerateNewIdentity { label }
                }
                IdentityDialogOutput::RenameIdentity { .. } => todo!(),
            },
        );

        let mut model = AppModel {
            identities_list: identities_list_component,
            messages: messages_component,
            network_status: network_status_component,
            stack: adw::ViewStack::default(),
            identity_dialog: identity_dialog_controller,
            show_plus_button: false,
        };

        let widgets = view_output!();
        match widgets.stack.visible_child_name().unwrap().as_str() {
            "identities" => model.show_plus_button = true,
            _ => model.show_plus_button = false,
        };
        model.stack = widgets.stack.clone();
        widgets
            .view_title
            .bind_property("title-visible", &widgets.view_bar, "reveal")
            .build();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppInput::PageChanged => match self.stack.visible_child_name().unwrap().as_str() {
                "identities" | "messages" => self.show_plus_button = true,
                _ => self.show_plus_button = false,
            },
            AppInput::HandleClickPlusButton => {
                match self.stack.visible_child_name().unwrap().as_str() {
                    "messages" => {
                        let mut message_composer = MessageComposer::builder().launch(()).detach();
                        message_composer.widget().present();
                        message_composer.detach_runtime();
                    }
                    "identities" => self.identity_dialog.widget().present(),
                    _ => {}
                }
            }
            AppInput::ShowPlusButton(v) => self.show_plus_button = v,
            AppInput::IdentitiesListUpdated => {
                self.messages.emit(MessagesInput::IdentitiesListUpdated)
            }
        }
    }
}
