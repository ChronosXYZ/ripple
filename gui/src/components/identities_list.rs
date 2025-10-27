use gtk::{self, prelude::*};
use relm4::factory::FactoryVecDeque;
use relm4::prelude::DynamicIndex;
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    loading_widgets::LoadingWidgets,
    view,
};
use relm4::{Component, ComponentController, Controller, RelmWidgetExt};

use crate::components::dialogs::identity_dialog::IdentityDialogOutput;

use crate::state;

use super::dialogs::identity_dialog::{IdentityDialogInit, IdentityDialogModel};
use super::factories::identity_list_row::{
    IdentityListRow, IdentityListRowInit, IdentityListRowInput,
};

//#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
//struct IdentityItem {
//    label: String,
//    address: String,
//}
//
//impl IdentityItem {
//    fn new(label: String, address: String) -> Self {
//        Self { label, address }
//    }
//}
//
//struct Widgets {
//    label: gtk::Label,
//}
//
//impl RelmListItem for IdentityItem {
//    type Root = gtk::Box;
//    type Widgets = Widgets;
//
//    fn setup(_item: &gtk::ListItem, column_index: usize) -> (gtk::Box, Widgets) {
//        relm4::view! {
//            my_box = gtk::Box {
//                #[name = "label"]
//                gtk::Label,
//            }
//        }
//
//        let widgets = Widgets { label };
//
//        (my_box, widgets)
//    }
//
//    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root, column_index: usize) {
//        let Widgets { label } = widgets;
//
//        if column_index == 0 {
//            label.set_label(&self.label);
//        } else if column_index == 1 {
//            label.set_label(&self.address);
//        }
//    }
//}

pub(crate) struct IdentitiesListModel {
    is_list_empty: bool,
    //list_view_wrapper: TypedListView<IdentityItem, gtk::SingleSelection, gtk::ColumnView>,
    identity_dialog: Controller<IdentityDialogModel>,
    list_view: FactoryVecDeque<IdentityListRow>,
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
}

#[derive(Debug)]
pub enum IdentitiesListOutput {
    EmptyList(bool),
    IdentitiesListUpdated,
}

impl IdentitiesListModel {
    async fn reload_list(&mut self, sender: relm4::AsyncComponentSender<Self>) {
        let identities = state::STATE
            .write_inner()
            .client
            .as_mut()
            .unwrap()
            .get_own_identities()
            .await;
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
    type CommandOutput = ();
    type Input = IdentitiesListInput;
    type Output = IdentitiesListOutput;
    type Init = ();

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

                    //#[local_ref]
                    //col_view -> gtk::ColumnView {
                    //    #[watch]
                    //    set_visible: !model.is_list_empty,
                    //}
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

    fn init_loading_widgets(root: &mut Self::Root) -> Option<LoadingWidgets> {
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
        _init: Self::Init,
        root: Self::Root,
        sender: relm4::AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        //let list_view_wrapper: TypedListView<IdentityItem, gtk::SingleSelection, gtk::ColumnView> =
        //    TypedListView::with_sorting_col(vec!["Label".to_string(), "Address".to_string()]);
        let list_view = gtk::ListBox::default();
        let list_view_factory = FactoryVecDeque::new(list_view.clone(), sender.input_sender());

        let mut model = Self {
            is_list_empty: true,
            list_view: list_view_factory,
            identity_dialog: Self::create_identity_dialog_controller(sender.clone(), None),
        };

        model.reload_list(sender.clone()).await;

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
                let address = state::STATE
                    .write_inner()
                    .client
                    .as_mut()
                    .unwrap()
                    .generate_new_identity(label.clone())
                    .await;
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
                state::STATE
                    .write_inner()
                    .client
                    .as_mut()
                    .unwrap()
                    .delete_identity(item.address)
                    .await;
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
                state::STATE
                    .write_inner()
                    .client
                    .as_mut()
                    .unwrap()
                    .rename_identity(address, new_label.clone())
                    .await;
                self.list_view
                    .send(index, IdentityListRowInput::RenameLabel(new_label));
                sender
                    .output(IdentitiesListOutput::IdentitiesListUpdated)
                    .unwrap();
            }
        }
    }
}
