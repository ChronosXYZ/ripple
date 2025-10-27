use adw;
use gtk::{self, prelude::*};
use relm4::{Component, ComponentParts, ComponentSender, RelmWidgetExt};
use relm4_icons::icon_name;

pub struct IdentityDialogModel {
    pub label: gtk::EntryBuffer,
    pub mode: IdentityDialogMode,
    pub button_label: String,
    pub address: String,
    pub index: Option<usize>,
}

pub struct IdentityDialogInit {
    pub label: String,
    pub address: String,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub enum IdentityDialogMode {
    New,
    Edit,
}

#[derive(Debug)]
pub enum IdentityDialogInput {
    HandleEntry,
}

#[derive(Debug)]
pub enum IdentityDialogOutput {
    GenerateIdentity(String),
    RenameIdentity {
        new_label: String,
        address: String,
        index: usize,
    },
}

#[relm4::component(pub)]
impl Component for IdentityDialogModel {
    type Input = IdentityDialogInput;
    type Output = IdentityDialogOutput;
    type Init = Option<IdentityDialogInit>;
    type CommandOutput = ();

    view! {
        #[root]
        adw::Window {
            set_hide_on_close: true,
            set_default_width: 320,
            set_resizable: false,
            set_modal: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    set_show_end_title_buttons: true,
                    set_css_classes: &["flat"],
                    set_title_widget: Some(&gtk::Box::default())
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 20,
                    set_spacing: 10,
                    gtk::Image {
                            set_icon_size: gtk::IconSize::Large,
                            set_icon_name: Some(match model.mode {
                                IdentityDialogMode::New => icon_name::PLUS,
                                IdentityDialogMode::Edit => icon_name::PENCIL_AND_PAPER
                            }),
                    },
                    gtk::Label {
                        set_css_classes: &["title-4"],
                        set_label: match model.mode {
                            IdentityDialogMode::New => "You're about to create an identity.",
                            IdentityDialogMode::Edit => "You're about to rename this identity."
                        },
                    },
                    gtk::Label {
                        set_label: "Pick a descriptive name.",
                    },
                    #[name = "new_list_entry"]
                    gtk::Entry {
                        set_placeholder_text: Some("Enter identity name..."),
                        set_buffer: &model.label,
                        connect_activate => IdentityDialogInput::HandleEntry,
                    },
                    gtk::Button {
                        set_css_classes: &["suggested-action"],
                        set_label: model.button_label.as_str(),
                        connect_clicked => IdentityDialogInput::HandleEntry,
                    },
                }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: &Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = if let Some(name) = init {
            IdentityDialogModel {
                label: gtk::EntryBuffer::new(Some(name.label)),
                mode: IdentityDialogMode::Edit,
                button_label: "Rename identity".to_string(),
                address: name.address,
                index: Some(name.index),
            }
        } else {
            IdentityDialogModel {
                label: gtk::EntryBuffer::new(Some("")),
                mode: IdentityDialogMode::New,
                button_label: "Create new identity".to_string(),
                address: "".to_string(),
                index: None,
            }
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match message {
            IdentityDialogInput::HandleEntry => {
                let name = self.label.text();

                match self.mode {
                    IdentityDialogMode::New => {
                        sender
                            .output(IdentityDialogOutput::GenerateIdentity(name.to_string()))
                            .unwrap_or_default();
                        self.label.set_text("");
                    }
                    IdentityDialogMode::Edit => {
                        sender
                            .output(IdentityDialogOutput::RenameIdentity {
                                new_label: name.to_string(),
                                address: self.address.clone(),
                                index: self.index.unwrap(),
                            })
                            .unwrap_or_default();
                    }
                }
                root.close();
            }
        }
    }
}
