mod utils;

use bollard::container::{ListContainersOptions, StatsOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use rtop_dev::components::listview::{ListItem, ListView, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

struct DockerContainersListWidget {
    process_items: Vec<ListItem>,
    process_list: ListView,
    docker: Option<Docker>,
    tokio_runtime: Option<Arc<Mutex<Runtime>>>,
}

impl rtop_dev::widget::Widget for DockerContainersListWidget {
    fn display(&mut self, height: i32, width: i32) -> String {
        self.process_list.resize(height, width);
        self.process_list.update_items(&self.process_items);
        self.process_list.display()
    }

    fn title(&mut self) -> Option<String> {
        Option::from("Docker Containers".to_owned())
    }

    fn on_update(&mut self) {
        let docker: Docker = self.docker.clone().unwrap();
        let runtime: Arc<Mutex<Runtime>> = self.tokio_runtime.as_ref().unwrap().clone();

        let containers_items_arc: Arc<tokio::sync::Mutex<Vec<ListItem>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let containers_items_mutex = std::sync::Arc::clone(&containers_items_arc);
        runtime.lock().unwrap().block_on(async move {
            let filter: HashMap<String, Vec<String>> = HashMap::new();
            let containers = &docker
                .list_containers(Some(ListContainersOptions {
                    all: true,
                    filters: filter,
                    ..Default::default()
                }))
                .await
                .unwrap();

            for container in containers {
                let container_id: &String = container.id.as_ref().unwrap();
                let stream = &mut docker
                    .stats(
                        container_id,
                        Some(StatsOptions {
                            stream: false,
                            ..Default::default()
                        }),
                    )
                    .take(1);

                while let Some(Ok(stats)) = stream.next().await {
                    let mut items: HashMap<String, String> = HashMap::new();
                    items.insert("ID".to_owned(), container_id.to_owned()[0..13].to_owned());
                    let container_status: String =
                        container.status.clone().unwrap_or_else(|| "".to_owned());
                    let container_status_clean: String =
                        if container_status.starts_with("Exited") {
                            "Exited"
                        } else if container_status.starts_with("Up") {
                            "Started"
                        } else if container_status.starts_with("Created") {
                            "Created"
                        } else if container_status.starts_with("Restarting") {
                            "Restarting"
                        } else if container_status.starts_with("Paused") {
                            "Paused"
                        } else if container_status.starts_with("Dead") {
                            "Dead"
                        } else {
                            "Unknown"
                        }
                        .to_owned();
                    items.insert("Status".to_owned(), container_status_clean);
                    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage
                        - stats.precpu_stats.cpu_usage.total_usage;
                    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0)
                        - stats.precpu_stats.system_cpu_usage.unwrap_or(0);
                    let result_cpu_usage = if system_delta == 0 || cpu_delta == 0 {
                        0
                    } else {
                        cpu_delta / system_delta * 100
                    };

                    items.insert("CPU".to_owned(), format!("{}%", result_cpu_usage));
                    items.insert(
                        "RAM".to_owned(),
                        utils::convert_to_readable_unity(
                            stats.memory_stats.usage.unwrap_or(0) as f64
                        ),
                    );
                    containers_items_mutex
                        .lock()
                        .await
                        .push(ListItem::new(stats.name.replace('/', "").as_str(), &items));
                }
            }
            self.process_items = containers_items_mutex.lock().await.to_vec();
        });
    }

    fn on_input(&mut self, key: String) {
        if key == "KEY_DOWN" {
            self.process_list.next();
        } else if key == "KEY_UP" {
            self.process_list.previous();
        } else if key == "m" {
            self.process_list.sort_by(
                Option::from(String::from("RAM")),
                Option::from(Ordering::Default),
            );
        } else if key == "n" {
            self.process_list.sort_by(
                Option::from(String::from("Name")),
                Option::from(Ordering::Inversed),
            );
        } else if key == "c" {
            self.process_list.sort_by(
                Option::from(String::from("CPU")),
                Option::from(Ordering::Default),
            );
        }
    }

    fn init(&mut self) {
        self.docker = Option::from(Docker::connect_with_socket_defaults().unwrap());
        self.tokio_runtime = Option::from(Arc::new(std::sync::Mutex::new(Runtime::new().unwrap())));
    }
}

#[no_mangle]
pub fn init_docker_containers() -> (Box<dyn rtop_dev::widget::Widget>, bool) {
    (
        Box::new(DockerContainersListWidget {
            process_items: Vec::new(),
            process_list: ListView::new(
                0,
                0,
                &Vec::new(),
                "Name".to_owned(),
                vec![
                    "ID".to_owned(),
                    "Status".to_owned(),
                    "RAM".to_owned(),
                    "CPU".to_owned(),
                ],
                Option::from(String::from("RAM")),
                Option::from(Ordering::Default),
            ),
            docker: None,
            tokio_runtime: None,
        }),
        true,
    )
}
