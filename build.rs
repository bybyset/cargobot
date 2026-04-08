use std::{fs, path::PathBuf};

fn main() {
    println!("cargo-warning=build.rs 开始执行，复制配置文件到构建输出目录");
    // build_copy_config_file();

    embuild::espidf::sysenv::output();
}

fn build_copy_config_file() {
    // 1. 确定目标输出目录
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    println!("cargo:warning=输出目录: {:?}", out_dir);

    
    // 目标: target/xtensa-esp32s3-espidf/release/out/
    let target_out = out_dir
        .parent().unwrap()   // build/esp-idf-sys-xxx
        .parent().unwrap()   // build
        .parent().unwrap()   // debug 或 release
        .parent().unwrap()   // xtensa-esp32s3-espidf
        .parent().unwrap()   // target
        .join("out");
    
    println!("cargo:warning=目标输出目录: {:?}", target_out);
    
    // 2. 确保目录存在
    fs::create_dir_all(&target_out).unwrap();
    
    // 3. 复制 partitions.csv
    let src_partitions = PathBuf::from("partitions.csv");
    let dst_partitions = target_out.join("partitions.csv");
    
    if src_partitions.exists() {
        match fs::copy(&src_partitions, &dst_partitions) {
            Ok(_) => println!("cargo:warning=已复制 partitions.csv 到 {:?}", dst_partitions),
            Err(e) => println!("cargo:warning=复制 partitions.csv 失败: {}", e),
        }
        println!("cargo:rerun-if-changed=partitions.csv");
    } else {
        println!("cargo:warning=找不到 partitions.csv 文件！");
    }
    
}
