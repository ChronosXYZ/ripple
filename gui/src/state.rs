use relm4::SharedState;

use crate::network::node::client::NodeClient;

pub(crate) static STATE: SharedState<GlobalAppState> = SharedState::new();

#[derive(Default)]
pub struct GlobalAppState {
    pub client: Option<NodeClient>,
}
