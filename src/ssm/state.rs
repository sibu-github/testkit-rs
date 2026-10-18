#[derive(Default)]
pub(crate) struct State {
    parameters: Vec<(String, String)>,
}

impl State {
    pub(crate) fn add_parameters(&mut self, params: &[(String, String)]) {
        for p in params {
            self.parameters.push(p.clone());
        }
    }

    pub(crate) fn parameters(&self) -> Vec<(String, String)> {
        self.parameters.clone()
    }
}
