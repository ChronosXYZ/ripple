use std::cell::{Ref, RefMut};

use gtk::{
    self, gio,
    glib::BoxedAnyObject,
    prelude::{Cast, CastNone, ListItemExt, ObjectExt, OrientableExt, WidgetExt},
};
use relm4::{
    component::{AsyncComponentParts, SimpleAsyncComponent},
    view, AsyncComponentSender,
};

use super::utils::typed_list_view::RelmListItem;
use crate::state;

#[derive(Debug, Clone)]
pub struct SelectedFolder {
    pub identity_address: String,
    pub folder: String,
}

#[derive(Debug)]
enum FolderItemType {
    Identity,
    Inbox,
    Sent,
}

#[derive(Debug)]
struct FolderItem {
    label: String,
    subtitle: String,
    item_type: FolderItemType,
}

struct IdentityItemWidgets {
    expander: gtk::TreeExpander,
    label: gtk::Label,
    subtitle: gtk::Label,
}

impl RelmListItem for FolderItem {
    type Root = gtk::TreeExpander;
    type Widgets = IdentityItemWidgets;

    fn setup(_list_item: &gtk::ListItem, _column_index: usize) -> (Self::Root, Self::Widgets) {
        view! {
            #[name(expander)]
            gtk::TreeExpander {
                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::Center,

                    #[name(label)]
                    gtk::Label {
                        set_halign: gtk::Align::Start,
                        set_valign: gtk::Align::Center
                    },
                    #[name(subtitle)]
                    gtk::Label {
                        add_css_class: "subtitle",
                        set_visible: false
                    }
                }
            }
        }

        let widgets = IdentityItemWidgets {
            expander: expander.clone(),
            label,
            subtitle,
        };
        (expander, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root, _column_index: usize) {
        widgets.label.set_text(&self.label);
        if let FolderItemType::Identity = self.item_type {
            widgets.subtitle.set_visible(true);
            widgets.subtitle.set_text(&self.subtitle);
        }
    }
}

pub struct MessagesSidebar {
    tree_model: gtk::TreeListModel,
}

#[derive(Debug)]
pub enum MessagesSidebarInput {
    IdentitiesListUpdated,
}

#[derive(Debug)]
pub enum MessagesSidebarOutput {
    FolderSelected(SelectedFolder),
}

#[relm4::component(pub async)]
impl SimpleAsyncComponent for MessagesSidebar {
    type Init = ();
    type Input = MessagesSidebarInput;
    type Output = MessagesSidebarOutput;

    view! {
        #[root]
        gtk::ScrolledWindow {
            set_width_request: 300,
            #[local_ref]
            list_view -> gtk::ListView {
                add_css_class: "navigation-sidebar",
            }
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let root_store = gio::ListStore::new::<BoxedAnyObject>();

        let identities = state::STATE
            .write_inner()
            .client
            .as_mut()
            .unwrap()
            .get_own_identities()
            .await;
        for i in identities {
            root_store.append(&BoxedAnyObject::new(FolderItem {
                label: if i.label.is_empty() {
                    "No label".to_string()
                } else {
                    i.label
                },
                subtitle: i.string_repr,
                item_type: FolderItemType::Identity,
            }))
        }

        let tree_model = gtk::TreeListModel::new(root_store.clone(), false, true, |o| {
            let boxed_object = o.clone().downcast::<BoxedAnyObject>().unwrap();
            let item: Ref<FolderItem> = boxed_object.borrow();
            if let FolderItemType::Identity = item.item_type {
                let inner_folders = gio::ListStore::new::<BoxedAnyObject>();
                inner_folders.append(&BoxedAnyObject::new(FolderItem {
                    label: "Inbox".to_string(),
                    subtitle: String::new(),
                    item_type: FolderItemType::Inbox,
                }));
                inner_folders.append(&BoxedAnyObject::new(FolderItem {
                    label: "Sent".to_string(),
                    subtitle: String::new(),
                    item_type: FolderItemType::Sent,
                }));
                return Some(inner_folders.upcast());
            }
            None
        });

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_factory, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let (root, widgets) = FolderItem::setup(item, 0);
            unsafe { root.set_data("widgets", widgets) };
            item.set_child(Some(&root));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let widget = list_item.child();

            let list_row = list_item
                .item()
                .unwrap()
                .downcast::<gtk::TreeListRow>()
                .unwrap();
            let obj = list_row
                .item()
                .unwrap()
                .downcast::<BoxedAnyObject>()
                .unwrap();
            let mut obj: RefMut<FolderItem> = obj.borrow_mut();
            let mut root = widget
                .and_downcast::<<FolderItem as RelmListItem>::Root>()
                .unwrap();

            let mut widgets = unsafe { root.steal_data("widgets") }.unwrap();
            if let FolderItemType::Identity = obj.item_type {
                list_item.set_activatable(false);
                list_item.set_selectable(false);
            }
            obj.bind(&mut widgets, &mut root, 0);
            widgets.expander.set_list_row(Some(&list_row));
            unsafe { root.set_data("widgets", widgets) };
        });

        let selection_model = gtk::SingleSelection::new(Some(tree_model.clone()));
        let list_view = gtk::ListView::new(Some(selection_model.clone()), Some(factory));

        selection_model.connect_selected_item_notify(move |sel_model| {
            if sel_model.selected_item().is_none() {
                return;
            }
            let tree_list_row = sel_model
                .selected_item()
                .unwrap()
                .downcast::<gtk::TreeListRow>()
                .unwrap();
            if tree_list_row.parent().is_none() {
                return;
            }
            let parent_boxed_data = tree_list_row
                .parent()
                .unwrap()
                .item()
                .unwrap()
                .downcast::<BoxedAnyObject>()
                .unwrap();
            let parent_of_selected_item: Ref<FolderItem> = parent_boxed_data.borrow();
            let boxed_obj = tree_list_row
                .item()
                .unwrap()
                .downcast::<BoxedAnyObject>()
                .unwrap();
            let selected_item: Ref<FolderItem> = boxed_obj.borrow();
            log::debug!(
                "Item selected: {:?}/{:?}",
                parent_of_selected_item,
                selected_item
            );
            let sender = sender.clone();
            sender
                .output(MessagesSidebarOutput::FolderSelected(SelectedFolder {
                    identity_address: parent_of_selected_item.subtitle.clone(),
                    folder: selected_item.label.clone(),
                }))
                .unwrap();
        });

        let model = Self { tree_model };

        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, message: Self::Input, _sender: AsyncComponentSender<Self>) {
        match message {
            MessagesSidebarInput::IdentitiesListUpdated => {
                let root_model = self
                    .tree_model
                    .model()
                    .downcast::<gio::ListStore>()
                    .unwrap();
                root_model.remove_all();
                let identities = state::STATE
                    .write_inner()
                    .client
                    .as_mut()
                    .unwrap()
                    .get_own_identities()
                    .await;
                for i in identities {
                    root_model.append(&BoxedAnyObject::new(FolderItem {
                        label: if i.label.is_empty() {
                            "No label".to_string()
                        } else {
                            i.label
                        },
                        subtitle: i.string_repr,
                        item_type: FolderItemType::Identity,
                    }))
                }
            }
        }
    }
}
