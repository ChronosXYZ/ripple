use adw;
use gtk::{self, prelude::*};
use relm4::component::{AsyncComponentController, AsyncController};
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    loading_widgets::LoadingWidgets,
    view,
};
use relm4::{AsyncComponentSender, RelmWidgetExt};

use super::messages_content::{MessagesContent, MessagesContentInput};
use super::messages_sidebar::{
    MessagesSidebar, MessagesSidebarInput, MessagesSidebarOutput, SelectedFolder,
};

pub(crate) struct MessagesModel {
    sidebar: AsyncController<MessagesSidebar>,
    content: AsyncController<MessagesContent>,
}

#[derive(Debug)]
pub(crate) enum MessagesInput {
    FolderSelected(SelectedFolder),
    IdentitiesListUpdated,
}

#[relm4::component(pub async)]
impl AsyncComponent for MessagesModel {
    type CommandOutput = ();
    type Input = MessagesInput;
    type Output = ();
    type Init = ();

    view! {
        #[root]
        gtk::ScrolledWindow {
            adw::Leaflet {
                model.sidebar.widget() -> &gtk::ScrolledWindow,
                gtk::Separator {},
                model.content.widget() -> &gtk::Box {}
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
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let sidebar = MessagesSidebar::builder()
            .launch(())
            .forward(sender.input_sender(), |msg| match msg {
                MessagesSidebarOutput::FolderSelected(v) => MessagesInput::FolderSelected(v),
            });
        let content = MessagesContent::builder().launch(()).detach();
        let model = Self { sidebar, content };
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            MessagesInput::FolderSelected(v) => {
                self.content.emit(MessagesContentInput::FolderSelected(v));
            }
            MessagesInput::IdentitiesListUpdated => self
                .sidebar
                .emit(MessagesSidebarInput::IdentitiesListUpdated),
        }
    }
}
