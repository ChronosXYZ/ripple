use gtk::{self, prelude::*};
use relm4::prelude::DynamicIndex;
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    loading_widgets::LoadingWidgets,
    view,
};
use relm4::{factory::FactoryVecDeque, AsyncComponentSender};
use relm4::{Component, ComponentController, Controller, RelmWidgetExt};
use ripple_core::network::address::Address;
use ripple_core::network::node::client::NodeClient;

use crate::components::{
    dialogs::identity_dialog::IdentityDialogOutput,
    factories::identity_list_row::IdentityListRowOutput,
};

use super::dialogs::identity_dialog::{IdentityDialogInit, IdentityDialogModel};
use super::factories::identity_list_row::{
    IdentityListRow, IdentityListRowInit, IdentityListRowInput,
};

pub(crate) struct IdentitiesListModel {
    is_list_empty: bool,
    //list_view_wrapper: TypedListView<IdentityItem, gtk::SingleSelection, gtk::ColumnView>,
    identity_dialog: Controller<IdentityDialogModel>,
    list_view: FactoryVecDeque<IdentityListRow>,
    node_client: NodeClient,
}

#[derive(Debug)]
pub enum IdentitiesListInput {
    HandleCreateNewIdentity,
    GenerateNewIdentity {
        label: String,
    },
    DeleteIdentity(DynamicIndex),
    HandleRenameIdentity(DynamicIndex),
    RenameIdentity {
        new_label: String,
        address: String,
        index: usize,
    },
    ReloadList,
}

#[derive(Debug)]
pub enum IdentitiesListOutput {
    EmptyList(bool),
    IdentitiesListUpdated,
}

#[derive(Debug)]
pub enum IdentitiesListCommandOutput {
    IdentitiesList(Vec<Address>),
}

impl IdentitiesListModel {
    fn create_identity_dialog_controller(
        sender: relm4::AsyncComponentSender<Self>,
        init: Option<IdentityDialogInit>,
    ) -> Controller<IdentityDialogModel> {
        IdentityDialogModel::builder()
            .launch(init)
            .forward(sender.input_sender(), |message| match message {
                IdentityDialogOutput::GenerateIdentity(label) => {
                    IdentitiesListInput::GenerateNewIdentity { label }
                }
                IdentityDialogOutput::RenameIdentity {
                    new_label,
                    address,
                    index,
                } => IdentitiesListInput::RenameIdentity {
                    new_label,
                    address,
                    index,
                },
            })
    }
}

#[relm4::component(pub async)]
impl AsyncComponent for IdentitiesListModel {
    type Input = IdentitiesListInput;
    type Output = IdentitiesListOutput;
    type Init = NodeClient;
    type CommandOutput = IdentitiesListCommandOutput;

    view! {
        #[root]
        gtk::ScrolledWindow {
            gtk::CenterBox {
                #[wrap(Some)]
                set_center_widget = &gtk::Box{
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        #[watch]
                        set_visible: model.is_list_empty,
                        set_spacing: 3,
                        set_valign: gtk::Align::Center,

                        gtk::Label {
                            set_label: "No identities yet :(",
                            add_css_class: "large-title"
                        },
                        gtk::Button {
                            set_label: "Create new one",
                            set_hexpand: false,
                            connect_clicked => IdentitiesListInput::HandleCreateNewIdentity
                        }
                    },

                    #[local]
                    list_view -> gtk::ListBox {
                        set_valign: gtk::Align::Start,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        add_css_class: "boxed-list",
                    }
                }
            }
        }
    }

    fn init_loading_widgets(root: Self::Root) -> Option<LoadingWidgets> {
        view! {
                #[local_ref]
                root {
                    #[name(loading)]
                    gtk::CenterBox {
                        set_margin_all: 100,
                        set_orientation: gtk::Orientation::Vertical,
                        #[wrap(Some)]
                        set_center_widget = &gtk::Spinner {
                            start: (),
                            set_size_request: (40, 40),
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                        },
                    }
                }
        }
        Some(LoadingWidgets::new(root, loading))
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: relm4::AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        //let list_view_wrapper: TypedListView<IdentityItem, gtk::SingleSelection, gtk::ColumnView> =
        //    TypedListView::with_sorting_col(vec!["Label".to_string(), "Address".to_string()]);
        let list_view = gtk::ListBox::default();
        let list_view_factory = FactoryVecDeque::builder()
            .launch(list_view.clone())
            .forward(sender.input_sender(), |output| match output {
                IdentityListRowOutput::DeleteIdentity(i) => IdentitiesListInput::DeleteIdentity(i),
                IdentityListRowOutput::RenameIdentity(i) => {
                    IdentitiesListInput::HandleRenameIdentity(i)
                }
            });

        let model = Self {
            is_list_empty: true,
            list_view: list_view_factory,
            identity_dialog: Self::create_identity_dialog_controller(sender.clone(), None),
            node_client: init,
        };

        sender.input(IdentitiesListInput::ReloadList);

        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        sender: relm4::AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            IdentitiesListInput::HandleCreateNewIdentity => {
                self.identity_dialog =
                    Self::create_identity_dialog_controller(sender.clone(), None);
                self.identity_dialog.widget().present();
            }
            IdentitiesListInput::GenerateNewIdentity { label } => {
                let address = self.node_client.generate_new_identity(label.clone()).await;
                self.list_view
                    .guard()
                    .push_back(IdentityListRowInit { label, address });
                if self.is_list_empty {
                    self.is_list_empty = false;
                    sender
                        .output(IdentitiesListOutput::EmptyList(false))
                        .unwrap();
                }
                sender
                    .output(IdentitiesListOutput::IdentitiesListUpdated)
                    .unwrap();
            }
            IdentitiesListInput::DeleteIdentity(i) => {
                let item = self
                    .list_view
                    .guard()
                    .remove(i.current_index())
                    .expect("identity to be existing");
                self.node_client.delete_identity(item.address).await;
                if self.list_view.len() == 0 {
                    self.is_list_empty = true;
                    sender
                        .output(IdentitiesListOutput::EmptyList(true))
                        .unwrap();
                }
                sender
                    .output(IdentitiesListOutput::IdentitiesListUpdated)
                    .unwrap();
            }
            IdentitiesListInput::HandleRenameIdentity(i) => {
                let guard = self.list_view.guard();
                let identity_item = guard
                    .get(i.current_index())
                    .expect("identity to be existing");

                self.identity_dialog = Self::create_identity_dialog_controller(
                    sender.clone(),
                    Some(IdentityDialogInit {
                        label: identity_item.label.clone(),
                        address: identity_item.address.clone(),
                        index: i.current_index(),
                    }),
                );
                self.identity_dialog.widget().present();
            }
            IdentitiesListInput::RenameIdentity {
                new_label,
                address,
                index,
            } => {
                self.node_client
                    .rename_identity(address, new_label.clone())
                    .await;
                self.list_view
                    .send(index, IdentityListRowInput::RenameLabel(new_label));
                sender
                    .output(IdentitiesListOutput::IdentitiesListUpdated)
                    .unwrap();
            }
            IdentitiesListInput::ReloadList => {
                let client = self.node_client.clone();
                sender.oneshot_command(async move {
                    let result = client.get_own_identities().await;
                    IdentitiesListCommandOutput::IdentitiesList(result)
                });
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _: &Self::Root,
    ) {
        match message {
            IdentitiesListCommandOutput::IdentitiesList(identities) => {
                if !identities.is_empty() {
                    self.is_list_empty = false;
                    sender
                        .output(IdentitiesListOutput::EmptyList(false))
                        .unwrap();
                } else {
                    self.is_list_empty = true;
                    sender
                        .output(IdentitiesListOutput::EmptyList(true))
                        .unwrap();
                }
                let mut guard = self.list_view.guard();
                guard.clear();
                for i in identities {
                    guard.push_back(IdentityListRowInit {
                        label: i.label,
                        address: i.string_repr,
                    });
                }
            }
        }
    }
}
