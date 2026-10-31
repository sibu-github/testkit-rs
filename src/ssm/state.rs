#[derive(Default)]
pub(crate) struct State {
    parameters: Vec<(String, String)>,
}

impl State {
    pub(crate) fn add_parameters(&mut self, params: &[(String, String)]) {
        for (name, value) in params {
            if let Some((_, existing_value)) = self
                .parameters
                .iter_mut()
                .find(|(existing_name, _)| existing_name == name)
            {
                *existing_value = value.clone();
            } else {
                self.parameters.push((name.clone(), value.clone()));
            }
        }
    }

    pub(crate) fn parameters(&self) -> Vec<(String, String)> {
        self.parameters.clone()
    }
}
