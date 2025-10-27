use std::cell::Ref;

use adw;
use gtk::{
    self, gio,
    glib::BoxedAnyObject,
    prelude::{Cast, CastNone, EntryBufferExtManual, ObjectExt, StaticType},
    traits::{
        ButtonExt, EntryExt, GridExt, GtkWindowExt, OrientableExt, TextBufferExt, TextViewExt,
        WidgetExt,
    },
};
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    view, AsyncComponentSender, RelmWidgetExt,
};

use crate::{components::utils::typed_list_view, state};

use super::utils::typed_list_view::RelmListItem;

#[derive(Debug, Clone)]
pub struct IdentityDropdownItem {
    label: String,
    address: String,
}

pub struct IdentityDropdownItemWidgets {
    label: gtk::Label,
}

impl RelmListItem for IdentityDropdownItem {
    type Root = gtk::Box;
    type Widgets = IdentityDropdownItemWidgets;

    fn setup(_list_item: &gtk::ListItem, _column_index: usize) -> (Self::Root, Self::Widgets) {
        view! {
            #[name(root)]
            gtk::Box {
                #[name(label)]
                gtk::Label {}
            }
        }
        let widgets = IdentityDropdownItemWidgets { label };
        (root, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root, _column_index: usize) {
        widgets.label.set_text(
            format!(
                "{} ({})",
                if self.label.is_empty() {
                    "No label"
                } else {
                    self.label.as_str()
                },
                self.address
            )
            .as_str(),
        )
    }
}

pub struct MessageComposer {
    current_identity: Option<IdentityDropdownItem>,
    to_buffer: gtk::EntryBuffer,
    subject_buffer: gtk::EntryBuffer,
    body_buffer: gtk::TextBuffer,
}

#[derive(Debug)]
pub enum MessageComposerInput {
    CancelButtonClicked,
    SendButtonClicked,
    IdentityItemSelected(IdentityDropdownItem),
}

#[relm4::component(pub async)]
impl AsyncComponent for MessageComposer {
    type Input = MessageComposerInput;
    type Output = ();
    type Init = ();
    type CommandOutput = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_default_size: (800, 600),
            set_title = Some(""),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    set_centering_policy: adw::CenteringPolicy::Strict,
                    set_show_end_title_buttons: false,
                    pack_start = &gtk::Button {
                        set_label: "Cancel",
                        connect_clicked => MessageComposerInput::CancelButtonClicked
                    },

                    pack_end = &gtk::Button {
                        #[watch]
                        set_sensitive: !model.current_identity.is_none(),
                        set_label: "Send",
                        add_css_class: "suggested-action",
                        connect_clicked => MessageComposerInput::SendButtonClicked
                    }
                },

                gtk::Grid {
                    set_margin_all: 10,
                    attach[0, 0, 2, 1] = &gtk::Label {
                        set_label: "From",
                        set_halign: gtk::Align::End
                    },
                    #[local_ref]
                    attach[3,0,1,1] = &dropdown -> gtk::DropDown {
                        set_hexpand: true
                    },
                    attach[0,1,2,1] = &gtk::Label {
                        set_halign: gtk::Align::End,
                        set_label: "To"
                    },
                    attach[3,1,1,1] = &gtk::Entry {
                        set_buffer: &model.to_buffer
                    },
                    attach[0,2,2,1] = &gtk::Label {
                        set_halign: gtk::Align::End,
                        set_label: "Subject"
                    },
                    attach[3,2,1,1] = &gtk::Entry {
                        set_buffer: &model.subject_buffer
                    },
                    set_column_spacing: 10,
                    set_row_spacing: 10,
                },
                gtk::Frame {
                    inline_css: "border-radius: 0px",
                    gtk::TextView {
                        set_left_margin: 5,
                        set_right_margin: 5,
                        set_top_margin: 5,
                        set_bottom_margin: 5,

                        set_editable: true,
                        set_monospace: true,
                        set_hexpand: true,
                        set_vexpand: true,
                        #[wrap(Some)]
                        set_buffer = &model.body_buffer.clone(),
                    }
                }
            }
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let mut model = MessageComposer {
            current_identity: None,
            to_buffer: gtk::EntryBuffer::new(Some("")),
            subject_buffer: gtk::EntryBuffer::new(Some("")),
            body_buffer: gtk::TextBuffer::new(None),
        };
        let identities = state::STATE
            .write_inner()
            .client
            .as_mut()
            .unwrap()
            .get_own_identities()
            .await;

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");

            let (root, widgets) = IdentityDropdownItem::setup(list_item, 0);
            unsafe { root.set_data("widgets", widgets) };
            list_item.set_child(Some(&root));
        });

        factory.connect_bind(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");

            let widget = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child();

            let obj = list_item.item().unwrap();
            let mut obj = typed_list_view::get_mut_value::<IdentityDropdownItem>(&obj);

            let mut root = widget
                .and_downcast::<<IdentityDropdownItem as RelmListItem>::Root>()
                .unwrap();

            let mut widgets = unsafe { root.steal_data("widgets") }.unwrap();
            obj.bind(&mut widgets, &mut root, 0);
            unsafe { root.set_data("widgets", widgets) };
        });

        let store = gio::ListStore::new(BoxedAnyObject::static_type());
        let items: Vec<IdentityDropdownItem> = identities
            .iter()
            .map(|x| IdentityDropdownItem {
                label: x.label.clone(),
                address: x.string_repr.clone(),
            })
            .collect();
        items.iter().for_each(|x| {
            store.append(&BoxedAnyObject::new(x.clone()));
        });

        let dropdown = gtk::DropDown::new(Some(store), gtk::Expression::NONE);
        dropdown.set_factory(Some(&factory));
        let s = sender.clone();
        dropdown.connect_selected_notify(move |x| {
            log::debug!("selected item");
            let obj = x
                .selected_item()
                .unwrap()
                .downcast::<BoxedAnyObject>()
                .unwrap();
            let item: Ref<IdentityDropdownItem> = obj.borrow();
            s.input(MessageComposerInput::IdentityItemSelected(item.clone()));
        });
        if !items.is_empty() {
            model.current_identity = Some(items[0].clone());
        }
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            MessageComposerInput::CancelButtonClicked => root.close(),
            MessageComposerInput::SendButtonClicked => {
                log::debug!(
                    "from: {:?}, to: {}, subject: {}, body: {}",
                    self.current_identity,
                    self.to_buffer.text(),
                    self.subject_buffer.text(),
                    self.body_buffer.text(
                        &self.body_buffer.start_iter(),
                        &self.body_buffer.end_iter(),
                        false
                    )
                );
                root.close();
                state::STATE
                    .write_inner()
                    .client
                    .as_mut()
                    .unwrap()
                    .send_message(
                        self.current_identity.as_ref().unwrap().address.clone(),
                        self.to_buffer.text().to_string(),
                        self.subject_buffer.text().to_string(),
                        self.body_buffer
                            .text(
                                &self.body_buffer.start_iter(),
                                &self.body_buffer.end_iter(),
                                false,
                            )
                            .to_string(),
                    )
                    .await;
            }
            MessageComposerInput::IdentityItemSelected(v) => self.current_identity = Some(v),
        }
    }
}
