use gtk::{self, prelude::*};
use relm4::RelmWidgetExt;
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    loading_widgets::LoadingWidgets,
    view,
};

pub(crate) struct NetworkStatusModel {}

#[derive(Debug)]
pub(crate) enum NetworkStatusInput {}

#[relm4::component(pub async)]
impl AsyncComponent for NetworkStatusModel {
    type CommandOutput = ();
    type Input = NetworkStatusInput;
    type Output = ();
    type Init = ();

    view! {
        #[root]
        gtk::ScrolledWindow {
            gtk::CenterBox {
                #[wrap(Some)]
                set_center_widget = &gtk::Label {
                    set_label: "Network Status is not implemented",
                    add_css_class: "large-title"
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
        _init: Self::Init,
        root: Self::Root,
        _sender: relm4::AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {};
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }
}
