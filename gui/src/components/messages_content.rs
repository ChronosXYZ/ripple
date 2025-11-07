use std::cell::Ref;

use chrono::Utc;
use gtk::{
    glib::BoxedAnyObject,
    prelude::{Cast, OrientableExt, TextBufferExt, TextViewExt, WidgetExt},
};
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    loading_widgets::LoadingWidgets,
    view, AsyncComponentSender, RelmWidgetExt,
};
use ripple_core::network::node::client::NodeClient;

use crate::network::node::worker::Folder;

use super::{
    messages_sidebar::SelectedFolder,
    utils::typed_list_view::{RelmListItem, TypedListView},
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct MessagesListItem {
    title: String,
    date: chrono::DateTime<Utc>,
    from: String,
    to: String,
    body: String,
    status: String,
}

pub struct MessagesListItemWidgets {
    label: gtk::Label,
}

impl RelmListItem for MessagesListItem {
    type Root = gtk::Box;
    type Widgets = MessagesListItemWidgets;

    fn setup(_list_item: &gtk::ListItem, _column_index: usize) -> (Self::Root, Self::Widgets) {
        view! {
            #[name(root)]
            gtk::Box{
                #[name(label)]
                gtk::Label {}
            }
        }

        let widgets = Self::Widgets { label };
        (root, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root, column_index: usize) {
        match column_index {
            0 => widgets
                .label
                .set_text(&self.date.format("%Y-%m-%d %H:%M:%S").to_string()), // Date
            1 => widgets.label.set_text(&self.from),  // From
            2 => widgets.label.set_text(&self.to),    // To
            3 => widgets.label.set_text(&self.title), // Title
            4 => widgets.label.set_text(&self.status), // Status
            _ => {}
        }
    }
}

pub struct MessagesContent {
    selected_folder: Option<SelectedFolder>,
    messages_list_view: TypedListView<MessagesListItem, gtk::SingleSelection, gtk::ColumnView>,
    current_msg: Option<MessagesListItem>,
    current_msg_buffer: gtk::TextBuffer,

    list_stack: gtk::Stack,
    node_client: NodeClient,
}

#[derive(Debug)]
pub enum MessagesContentInput {
    FolderSelected(SelectedFolder),
    MessageSelected(MessagesListItem),
}

#[relm4::component(pub async)]
impl AsyncComponent for MessagesContent {
    type Init = NodeClient;
    type Input = MessagesContentInput;
    type Output = ();
    type CommandOutput = ();

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            match model.selected_folder.clone() {
                Some(_) => {
                    #[name(list_stack)]
                    gtk::Stack {
                        set_vexpand: true,

                        add_named[Some("list")] = &gtk::Paned {
                            set_margin_all: 12,
                            set_orientation: gtk::Orientation::Vertical,

                            #[wrap(Some)]
                            set_start_child = &gtk::Frame {
                                gtk::ScrolledWindow {
                                    #[local_ref]
                                    messages_list -> gtk::ColumnView {},
                                }
                            },
                            #[wrap(Some)]
                            set_end_child = &gtk::Frame {
                                #[name(message_text_view)]
                                gtk::TextView {
                                    set_left_margin: 5,
                                    set_right_margin: 5,
                                    set_top_margin: 5,
                                    set_bottom_margin: 5,

                                    set_editable: false,
                                    set_cursor_visible: false,

                                    #[wrap(Some)]
                                    set_buffer = &model.current_msg_buffer.clone(),
                                }
                            },
                        },
                        add_named[Some("empty")] = &gtk::Label {
                            set_vexpand: true,
                            set_label: "No messages in the folder :(",
                            add_css_class: "large-title"
                        },

                        set_visible_child_name: "empty",
                    }
                },
                None => {
                    gtk::Label {
                        set_vexpand: true,
                        set_label: "Select folder to view messages",
                        add_css_class: "large-title"
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
        node_client: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let messages_list_view: TypedListView<MessagesListItem, gtk::SingleSelection, _> =
            TypedListView::with_sorting_col(vec![
                "Date".to_string(),
                "From".to_string(),
                "To".to_string(),
                "Title".to_string(),
                "Status".to_string(),
            ]);

        messages_list_view
            .selection_model
            .connect_selected_item_notify(move |sel_model| {
                let sender = sender.clone();
                if sel_model.selected_item().is_none() {
                    return;
                }
                let boxed_data = sel_model
                    .selected_item()
                    .unwrap()
                    .downcast::<BoxedAnyObject>()
                    .unwrap();
                let selected_item: Ref<MessagesListItem> = boxed_data.borrow();
                sender.input(MessagesContentInput::MessageSelected(selected_item.clone()));
            });

        let mut model = Self {
            selected_folder: None,
            messages_list_view,
            current_msg: None,
            current_msg_buffer: gtk::TextBuffer::new(None),
            list_stack: gtk::Stack::default(),
            node_client,
        };

        let messages_list = &model.messages_list_view.view;
        let widgets = view_output!();
        model.list_stack = widgets.list_stack.clone();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            MessagesContentInput::FolderSelected(selected_folder) => {
                self.messages_list_view.clear();
                self.selected_folder = Some(selected_folder.clone());
                let folder = match selected_folder.folder.as_str() {
                    "Inbox" => Folder::Inbox,
                    "Sent" => Folder::Sent,
                    _ => Folder::Inbox,
                };
                // load messages from db
                let msgs = self
                    .node_client
                    .get_messages(selected_folder.identity_address.clone(), folder)
                    .await;
                if !msgs.is_empty() {
                    self.list_stack.set_visible_child_name("list");
                    for m in msgs {
                        let mime_msg = mail_parser::Message::parse(m.data.as_slice()).unwrap();
                        let title = mime_msg.subject().unwrap().to_string();
                        let date = m.created_at;
                        let from = m.sender;
                        let body = mime_msg.body_text(0).unwrap();
                        self.messages_list_view.append(MessagesListItem {
                            title,
                            date,
                            from,
                            to: m.recipient,
                            body: body.to_string(),
                            status: m.status,
                        });
                    }
                } else {
                    self.list_stack.set_visible_child_name("empty");
                }
            }
            MessagesContentInput::MessageSelected(m) => {
                self.current_msg = Some(m.clone());
                self.current_msg_buffer.set_text(m.body.as_str());
            }
        }
    }
}
