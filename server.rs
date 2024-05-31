use {
    crate::PluginData,
    chrono::{DateTime, Duration, Utc},
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc},
    tokio::{
        fs::{self, read_dir, File},
        io::AsyncReadExt,
    },
    types::{api::{APIError, CompressedEvent}, timing::TimeRange},
};

#[derive(Deserialize)]
struct ConfigData {
    pub usage_files: PathBuf,
    pub apps_file: PathBuf
}

pub struct Plugin {
    _plugin_data: PluginData,
    config: ConfigData,
    apps_map: Arc<AppsMap>
}

impl crate::Plugin for Plugin {
    async fn new(data: PluginData) -> Self
    where
        Self: Sized,
    {
        let config: ConfigData = toml::Value::try_into(
            data.config
                .clone()
                .expect("Failed to init usage plugin! No config was provided!"),
        )
        .unwrap_or_else(|e| {
            panic!(
                "Unable to init usage plugin! Provided config does not fit the requirements: {}",
                e
            )
        });

        let apps_map = match AppsMap::new(&config.apps_file).await {
            Ok(v) => v,
            Err(e) => {
                panic!("Unable to init app names lookup table: {}", e);
            }
        };

        Plugin {
            _plugin_data: data,
            config,
            apps_map: Arc::new(apps_map)
        }
    }

    fn get_type() -> types::api::AvailablePlugins
    where
        Self: Sized,
    {
        types::api::AvailablePlugins::timeline_plugin_usage
    }

    fn get_compressed_events(
        &self,
        query_range: &types::timing::TimeRange,
    ) -> std::pin::Pin<
        Box<
            dyn futures::Future<Output = types::api::APIResult<Vec<types::api::CompressedEvent>>>
                + Send,
        >,
    > {
        let query_range = query_range.clone();
        let files = self.config.usage_files.clone();
        let apps_map = self.apps_map.clone();
        Box::pin(async move {
            let res = match Plugin::get_eventerized_usage_statistics(files, &query_range, apps_map).await {
                Ok(v) => v,
                Err(e) => return Err(APIError::Custom(e))
            };

            Ok(res)
        })
    }
}

#[derive(Serialize)]
struct AppEvent {
    pub app: String,
    pub duration: u64
}

struct UsageStatistic {
    timing: TimeRange,
    usage_statistic: HashMap<String, Duration>
}

impl Plugin {

    async fn get_eventerized_usage_statistics (files: PathBuf, range: &TimeRange, apps_map: Arc<AppsMap>) ->  Result<Vec<CompressedEvent>, String> {
        let data = Plugin::collect_data(files, range).await?;
        let statistics = Plugin::generate_usage_statistics(data, range.start, Duration::hours(1))?;

        let mut resulting_events = Vec::new();

        for statistic in statistics {
            for (app, duration) in statistic.usage_statistic {
                let app_name = match apps_map.get_app_name(&app).await {
                    Some(v) => v.clone(),
                    None => app
                };
                resulting_events.push(CompressedEvent {
                    time: types::timing::Timing::Range(statistic.timing.clone()),
                    title: app_name.clone(),
                    data: Box::new(AppEvent {
                        app: app_name,
                        duration: duration.num_minutes() as u64
                    }),
                })
            }
        }


        Ok(resulting_events)
    }

    fn generate_usage_statistics (data: Vec<UsageEvent>, start: DateTime<Utc>, time_step: Duration) -> Result<Vec<UsageStatistic>, String> {
        let mut current_time = start;
        let mut result = vec![UsageStatistic {
            usage_statistic: HashMap::new(),
            timing: TimeRange { start: current_time, end: current_time + time_step }
        }];

        let mut iterator = data.into_iter().peekable();

        while let Some(data_point) = iterator.next() {
            let updated_app = match data_point.change {
                UsageEventChange::StopUsing => {
                    continue;
                }
                UsageEventChange::StartUsing(app) => {
                    app
                } 
            };

            if data_point.time >= current_time + time_step {
                current_time += time_step;
                result.push(UsageStatistic {
                    usage_statistic: HashMap::new(),
                    timing: TimeRange { start: current_time, end: current_time + time_step }
                })
            }
            else {
                let statistic = &mut result.last_mut().unwrap().usage_statistic;
                let used_for = match iterator.peek() {
                    Some(next_event) => {
                        next_event.time - data_point.time
                    },
                    None => {
                        continue;
                    }
                };
                #[allow(clippy::map_entry)]
                if statistic.contains_key(&updated_app) {
                    statistic.get_mut(&updated_app).unwrap().checked_add(&used_for).unwrap();
                }
                else {
                    statistic.insert(updated_app, used_for);
                }
            }
        }

        Ok(result)
    }

    async fn collect_data(files: PathBuf, range: &TimeRange) -> Result<Vec<UsageEvent>, String> {
        let mut dir = match read_dir(&files).await {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("Unable to read usage files: {}", e));
            }
        };

        let mut dir_entries = Vec::new();

        while let Some(v) = match dir.next_entry().await {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("Unable to read next directory entry: {}", e));
            }
        } {
            let filename = match v.file_name().into_string() {
                Ok(v) => v,
                Err(e) => {
                    return Err(format!(
                        "Unable to read dir. Name is invalid string: {:?}",
                        e
                    ))
                }
            };
            dir_entries.push((string_timestamp_to_datetime(&filename)?, v.path()));
        }

        dir_entries.sort_by(|a, b| {a.0.cmp(&b.0)});

        let mut usage = Vec::new();

        let mut dir_entries_iterator = dir_entries.into_iter().peekable();

        'file_loop: while let Some(entry) = dir_entries_iterator.next() {
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
            if entry.0 < range.start {
                let theo_next = match dir_entries_iterator.peek() {
                    Some(v) => v,
                    None => continue,
                };
                if theo_next.0 > range.start {
                    let data_in_file = Plugin::read_file(&entry.1).await?;
                    for data in data_in_file {
                        if range.includes(&data.time) {
                            usage.push(data);
                        }
                    }
                } else {
                    continue;
                }
            } else if entry.0 <= range.end {
                let data_in_file = Plugin::read_file(&entry.1).await?;
                for data in data_in_file {
                    if range.includes(&data.time) {
                        usage.push(data)
                    } else if data.time > range.end {
                        break 'file_loop;
                    }
                }
            } else {
                break;
            }
        }

        Ok(usage)
    }

    async fn read_file(path: &Path) -> Result<Vec<UsageEvent>, String> {
        let content = match fs::File::open(path).await {
            Ok(mut v) => {
                let mut str = String::new();
                if let Err(e) = v.read_to_string(&mut str).await {
                    return Err(format!(
                        "Unable tor read usage file to string. Path: {} \nError: {}",
                        path.display(),
                        e
                    ));
                }
                str
            }
            Err(e) => {
                return Err(format!(
                    "Unable to read usage file. Path: {} \nError: {}",
                    path.display(),
                    e
                ));
            }
        };

        Ok(content
            .split('\n')
            .flat_map(|v| {
                let mut split = v.split(':');
                let time = split.next();
                let time = match time {
                    Some(v) => match v.parse() {
                        Ok(v) => Some(
                            DateTime::<Utc>::from_timestamp(v, 0)
                                .expect("Invalid timestamp in file?"),
                        ),
                        Err(_e) => None,
                    },
                    None => None,
                };
                let action = split.next();
                let app = split.next();
                match (time, action, app) {
                    (Some(t), Some("open"), Some(app)) => Some(UsageEvent {
                        time: t,
                        change: UsageEventChange::StartUsing(app.to_string()),
                    }),
                    (Some(t), Some("lock"), _) => Some(UsageEvent {
                        time: t,
                        change: UsageEventChange::StopUsing,
                    }),
                    _ => None,
                }
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
struct UsageEvent {
    time: DateTime<Utc>,
    change: UsageEventChange,
}

#[derive(Debug, Clone)]
enum UsageEventChange {
    StartUsing(String),
    StopUsing,
}

fn string_timestamp_to_datetime(timestamp: &str) -> Result<DateTime<Utc>, String> {
    match DateTime::<Utc>::from_timestamp(
        match timestamp.parse() {
            Ok(v) => v,
            Err(e) => return Err(format!("Unable to parse timestamp not a number: {}", e)),
        },
        0,
    ) {
        Some(v) => Ok(v),
        None => Err("Timestamp can't be converted to DateTime".to_string()),
    }
}

struct AppsMap {
    apps_map: HashMap<String, String>
}

impl AppsMap {
    pub async fn new (path: &Path) -> Result<AppsMap, String> {
        let apps_map = match File::open(path).await {
            Ok(mut v) => {  
                let mut str = String::new();
                if let Err(e) = v.read_to_string(&mut str).await {
                    return Err(format!("Error reading apps file: {}", e));
                }

                str.split('\n').filter_map(|line| {
                    line.split_once(':').map(|v| (v.0.to_string(), v.1.to_string()))
                }).collect()
            },
            Err(e) => {
                return Err(format!("Error reading apps file: {}", e));
            }
        };

        Ok(AppsMap { apps_map })
    }

    pub async fn get_app_name (&self, package: &str) -> Option<&String> {
        self.apps_map.get(package)
    }
}