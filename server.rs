use {
    crate::PluginData,
    chrono::{DateTime, TimeDelta},
    serde::Deserialize,
    serde_json::json,
    types::{api::CompressedEvent, timing::TimeRange},
    url::Url,
    std::path::PathBuf,
    tokio::fs::read_dir
};

#[derive(Deserialize)]
struct ConfigData{
    pub usage_files: PathBuf
}

pub struct Plugin {
    plugin_data: PluginData,
    config: ConfigData
}

impl crate::Plugin for Plugin {
    async fn new(data: PluginData) -> Self
    where
        Self: Sized,
    {
        let config: ConfigData = toml::Value::try_into(
            data.config
                .clone().expect("Failed to init usage plugin! No config was provided!")
        )
        .unwrap_or_else(|e| panic!("Unable to init usage plugin! Provided config does not fit the requirements: {}", e));

        Plugin { plugin_data: data, config }
    }

    fn get_type() -> types::api::AvailablePlugins
    where
        Self: Sized,
    {
        types::api::AvailablePlugins::timeline_plugin_usage
    }

    fn get_compressed_events (&self, query_range: &types::timing::TimeRange) -> std::pin::Pin<Box<dyn futures::Future<Output = types::api::APIResult<Vec<types::api::CompressedEvent>>> + Send>> {
        let usage_files = self.config.usage_files.clone();
        let query_range = query_range.clone();
        Box::pin(async move {
            let resulting_vec = vec![];
            
            Ok(resulting_vec)
        })
    }

}

impl Plugin {
    async fn collect_data(&self, range: TimeRange) -> Result<Vec<Usage>, String> {
        let mut dir = match read_dir(&self.config.usage_files).await {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("Unable to read usage files: {}", e));
            }
        };

        let mut usage = Vec::new();

        while let Some(v) = match dir.next_entry().await {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("Unable to read next directory entry"));
            }
        } {
            //check if filename is smaller than range start
                //then check if next filename is bigger than start (iterator.peek)
                    //read file and skip lines until the times are bigger than start. Now push the lines to usage as long as the times are smaller than end
                //else
                    //continue
            //else check if filename is smaller than end
                //read file until times are bigger than end
                    //if times are actually bigger than end
                        //break
            //else
                //break
        }
    
        Ok(usage)
    }
}

#[derive(Debug, Clone)]
enum Usage {
    StartUsing(u32, String),
    StopUsing(u32)
}