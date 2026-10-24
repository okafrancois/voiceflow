use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct ModelFile {
    pub name: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    pub size: &'static str,
    pub repository: &'static str,
    pub revision: &'static str,
    pub files: &'static [ModelFile],
}

const QWEN_06_FILES: &[ModelFile] = &[
    ModelFile {
        name: "added_tokens.json",
        bytes: 707,
        sha256: "c0284b582e14987fbd3d5a2cb2bd139084371ed9acbae488829a1c900833c680",
    },
    ModelFile {
        name: "config.json",
        bytes: 937,
        sha256: "15d3ac26c043ae477273ed5802ee0f0b33bb14f18c9d3dd70910c02d906e3f1f",
    },
    ModelFile {
        name: "merges.txt",
        bytes: 1671853,
        sha256: "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5",
    },
    ModelFile {
        name: "model.safetensors",
        bytes: 335450584,
        sha256: "392e8d466d56100ada00eb82031fb854297fc9e389b7d303eba3af114e87bce2",
    },
    ModelFile {
        name: "model.safetensors.index.json",
        bytes: 49731,
        sha256: "7b294141456f6904936db03c00bca50fb5f6198f652fe8483f9cd2a1018accfb",
    },
    ModelFile {
        name: "special_tokens_map.json",
        bytes: 613,
        sha256: "76862e765266b85aa9459767e33cbaf13970f327a0e88d1c65846c2ddd3a1ecd",
    },
    ModelFile {
        name: "tokenizer.json",
        bytes: 11422654,
        sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
    },
    ModelFile {
        name: "tokenizer_config.json",
        bytes: 9706,
        sha256: "253153d0738ceb4c668d2eff957714dd2bea0b56de772a9fdccd96cbf517e6a0",
    },
    ModelFile {
        name: "vocab.json",
        bytes: 2776833,
        sha256: "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
    },
];

const QWEN_17_FILES: &[ModelFile] = &[
    ModelFile {
        name: "added_tokens.json",
        bytes: 707,
        sha256: "c0284b582e14987fbd3d5a2cb2bd139084371ed9acbae488829a1c900833c680",
    },
    ModelFile {
        name: "config.json",
        bytes: 937,
        sha256: "507a6701220524eb8b283425bf0856a9ae4f21f4052e563896ddd668994b1dc7",
    },
    ModelFile {
        name: "merges.txt",
        bytes: 1671853,
        sha256: "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5",
    },
    ModelFile {
        name: "model.safetensors",
        bytes: 968080210,
        sha256: "0e86d9677e519323849eac1bc272caae88567a481ff188c431f70be543d9995f",
    },
    ModelFile {
        name: "model.safetensors.index.json",
        bytes: 49731,
        sha256: "1e3058d4ba4b04e4de35b74467725cbef90ff022198404218e48f21adc9cfa15",
    },
    ModelFile {
        name: "special_tokens_map.json",
        bytes: 613,
        sha256: "76862e765266b85aa9459767e33cbaf13970f327a0e88d1c65846c2ddd3a1ecd",
    },
    ModelFile {
        name: "tokenizer.json",
        bytes: 11422654,
        sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
    },
    ModelFile {
        name: "tokenizer_config.json",
        bytes: 9706,
        sha256: "253153d0738ceb4c668d2eff957714dd2bea0b56de772a9fdccd96cbf517e6a0",
    },
    ModelFile {
        name: "vocab.json",
        bytes: 2776833,
        sha256: "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
    },
];

pub const MODELS: &[Model] = &[
    Model {
        id: "mlx-qwen3-0.6b",
        name: "Qwen3 0.6B · MLX",
        size: "351 MB",
        repository: "mlx-community/Qwen3-0.6B-4bit",
        revision: "73e3e38d981303bc594367cd910ea6eb48349da8",
        files: QWEN_06_FILES,
    },
    Model {
        id: "mlx-qwen3-1.7b",
        name: "Qwen3 1.7B · MLX",
        size: "984 MB",
        repository: "mlx-community/Qwen3-1.7B-4bit",
        revision: "3b1b1768f8f8cf8351c712464f906e86c2b8269e",
        files: QWEN_17_FILES,
    },
];

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|model| model.id == id)
}

pub fn for_platform(apple_silicon: bool) -> &'static [Model] {
    if apple_silicon {
        MODELS
    } else {
        &[]
    }
}

pub fn files_complete(models_dir: &Path, id: &str) -> bool {
    let Some(model) = find(id) else {
        return false;
    };
    files_complete_for(models_dir, model)
}

pub(super) fn files_complete_for(models_dir: &Path, model: &Model) -> bool {
    let directory = models_dir.join(model.id);
    if !std::fs::symlink_metadata(&directory)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return false;
    }
    if !std::fs::read_to_string(directory.join(".model-ready"))
        .is_ok_and(|value| value == signature(model))
    {
        return false;
    }
    model.files.iter().all(|file| {
        std::fs::symlink_metadata(directory.join(file.name)).is_ok_and(|metadata| {
            metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() == file.bytes
        })
    })
}

pub fn downloaded(models_dir: &Path, id: &str) -> bool {
    super::runtime::installed(models_dir) && files_complete(models_dir, id)
}

pub(super) fn signature(model: &Model) -> String {
    format!(
        "{}\n{}\n{}",
        model.repository,
        model.revision,
        model
            .files
            .iter()
            .map(|file| format!("{} {} {}\n", file.name, file.bytes, file.sha256))
            .collect::<String>()
    )
}
