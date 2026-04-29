# 唤醒词模型

这个目录用于存放 Rustpotter 唤醒词模型文件（.rpw）。

## 如何获取模型

1. 使用 Rustpotter CLI 工具训练自己的唤醒词模型
2. 或者使用预训练的模型

## 训练模型

安装 Rustpotter CLI：

```bash
cargo install rustpotter-cli
```

训练唤醒词：

```bash
rustpotter-cli train --name "xiaoliang" --samples ./samples --output ./resources/wake_word.rpw
```

## 模型配置

- 采样率: 16000 Hz
- 位深度: 16-bit
- 声道数: 1（单声道）

## 注意事项

- 确保模型文件命名为 `wake_word.rpw`
- 模型文件大小建议不超过 500KB 以适应 ESP32-S3 的 Flash 容量
