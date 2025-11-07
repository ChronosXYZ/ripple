use std::path::PathBuf;

use crate::{components::message_composer::MessageComposerOutput, icon_names};

use relm4::{
    component::{AsyncComponent, AsyncComponentController, AsyncController},
    prelude::SimpleAsyncComponent,
};
use relm4::{gtk::prelude::*, prelude::AsyncComponentParts};
use relm4::{AsyncComponentSender, Component, ComponentController, Controller};
use ripple_core::network::{self, node::client::NodeClient};

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
    node_client: NodeClient,
    toast_overlay: adw::ToastOverlay,
}

#[derive(Debug)]
pub(crate) enum AppInput {
    PageChanged,
    HandleClickPlusButton,
    ShowPlusButton(bool),
    IdentitiesListUpdated,
    ShowToast(String),
}

pub struct AppModelInit {
    pub data_dir: PathBuf,
}

#[relm4::component(pub async)]
impl SimpleAsyncComponent for AppModel {
    type Input = AppInput;
    type Output = ();
    type Init = AppModelInit;

    view! {
        adw::ApplicationWindow {
            set_default_size: (800, 600),

            set_title = Some("Ripple"),

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    set_centering_policy: adw::CenteringPolicy::Strict,

                    #[wrap(Some)]
                    set_title_widget = &adw::ViewSwitcher {
                        set_stack: Some(&stack),
                        set_policy: adw::ViewSwitcherPolicy::Wide,
                    },
                    pack_start = if model.show_plus_button {
                        gtk::Button{
                            set_icon_name: icon_names::PLUS,
                            connect_clicked => AppInput::HandleClickPlusButton
                        }
                    } else { gtk::Box{} }
                },

                #[wrap(Some)]
                #[name="toast_overlay"]
                set_content = &adw::ToastOverlay {
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_vexpand: true,

                        #[name="stack"]
                        adw::ViewStack {
                            set_vexpand: true,

                            connect_visible_child_name_notify => AppInput::PageChanged,

                            add_titled[Some("identities"), "Identities"] = model.identities_list.widget() -> &gtk::ScrolledWindow{} -> {
                                set_icon_name: Some(icon_names::PERSON),
                            },

                            add_titled[Some("messages"), "Messages"] = model.messages.widget() -> &gtk::ScrolledWindow {} -> {
                                set_icon_name: Some(icon_names::MAIL_INBOX_FILLED),
                            },

                            add_titled[Some("status"), "Network Status"] = model.network_status.widget() -> &gtk::ScrolledWindow {} -> {
                                set_icon_name: Some(icon_names::DESKTOP_PULSE_FILLED),
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
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (node_client, worker) = network::new(None, init.data_dir.clone()).await;

        let _worker_handle = tokio::spawn(worker.run());

        if let Err(e) = node_client
            .start_listening("/ip4/0.0.0.0/tcp/34064".parse().unwrap())
            .await
        {
            log::error!("Failed to start listening: {}", e);
        }

        let identities_list_component = IdentitiesListModel::builder()
            .launch(node_client.clone())
            .forward(sender.input_sender(), |message| match message {
                IdentitiesListOutput::EmptyList(v) => AppInput::ShowPlusButton(!v),
                IdentitiesListOutput::IdentitiesListUpdated => AppInput::IdentitiesListUpdated,
            });
        let messages_component = MessagesModel::builder()
            .launch(node_client.clone())
            .detach();
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
            node_client,
            toast_overlay: adw::ToastOverlay::new(),
        };

        let widgets = view_output!();
        match widgets.stack.visible_child_name().unwrap().as_str() {
            "identities" => model.show_plus_button = true,
            _ => model.show_plus_button = false,
        };
        model.stack = widgets.stack.clone();
        model.toast_overlay.clone_from(&widgets.toast_overlay);

        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, message: Self::Input, sender: AsyncComponentSender<Self>) {
        match message {
            AppInput::PageChanged => match self.stack.visible_child_name().unwrap().as_str() {
                "identities" | "messages" => self.show_plus_button = true,
                _ => self.show_plus_button = false,
            },
            AppInput::HandleClickPlusButton => {
                match self.stack.visible_child_name().unwrap().as_str() {
                    "messages" => {
                        let mut message_composer = MessageComposer::builder()
                            .launch(self.node_client.clone())
                            .forward(sender.input_sender(), |message| match message {
                                MessageComposerOutput::FailedToSendMessage(e) => {
                                    AppInput::ShowToast(format!("Failed to send message: {}", e))
                                }
                            });
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
            AppInput::ShowToast(text) => {
                let toast = adw::Toast::new(&text);
                toast.set_timeout(3);
                self.toast_overlay.add_toast(toast);
            }
        }
    }
}
