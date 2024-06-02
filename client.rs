use {
    leptos::{view, View, IntoView},
    serde::Deserialize
};

pub struct Plugin {
    
}

impl crate::plugin_manager::Plugin for Plugin {
    fn get_style(&self) -> crate::plugin_manager::Style {
        crate::plugin_manager::Style::Acc1
    }

    async fn new(_data: crate::plugin_manager::PluginData) -> Self
        where
            Self: Sized {
        Plugin {}
    }

    fn get_component(&self, data: crate::plugin_manager::PluginEventData) -> crate::event_manager::EventResult<Box<dyn FnOnce() -> leptos::View>> {
        let data = data.get_data::<AppEvent>()?;
        Ok(Box::new(
            move || -> View {
                view! {
                    <div style="display: flex; flex-direction: row; width: 100%; gap: calc(var(--contentSpacing) * 0.5); background-color: var(--accentColor1);align-items: start;">
                        <img
                            style="width: calc(var(--contentSpacing) * 5); aspect-ratio: 1; padding: var(--contentSpacing);"
                            src=move || {
                                crate::api::relative_url(
                                        &format!(
                                            "/api/plugin/timeline_plugin_usage/icon/{}",
                                            data.package,
                                        ),
                                    )
                                    .unwrap()
                                    .to_string()
                            }
                        />

                        <div style="padding-top: calc(var(--contentSpacing) * 0.5); padding-bottom: calc(var(--contentSpacing) * 0.5); color: var(--lightColor); overflow: hidden;">
                            <h3>{move || { data.app.clone() }}</h3>
                            <a>{move || { format!("{}m", data.duration) }}</a>
                        </div>
                    </div>
                }.into_view()
            }
        ))
    }
}

#[derive(Deserialize)]
struct AppEvent {
    pub app: String,
    pub duration: u64,
    pub package: String
}