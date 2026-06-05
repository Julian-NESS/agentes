use serde_json::{json, Value};

fn main() {
    let mut data = json!({
        "huawei_specific": {
            "mem_usage_percent": 30
        },
        "performance": {
            "memory": {
                "mem_usage_percent": 0.0
            }
        }
    });

    if let Some(vendor) = data.get("huawei_specific") {
        let mem_pct = vendor.get("mem_usage_percent").cloned();
        if let Some(perf) = data.get_mut("performance") {
            if let Some(mem) = perf.get_mut("memory") {
                if let Some(p) = mem_pct {
                    mem["mem_usage_percent"] = p;
                }
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&data).unwrap());
}
