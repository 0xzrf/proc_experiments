use builder::Builder;

#[derive(Builder, PartialEq, Debug)]
pub struct BuilderStruct {
    pub x: u64,
    #[builder(each = "arg")]
    pub args: Vec<String>,
    pub executable: String,
}

pub fn handle_bulider() {
    let mut builder_instance = BuilderStruct {
        x: 1,
        executable: "Something".to_string(),
        args: vec!["elem".to_string()],
    };

    builder_instance
        .x(4)
        .executable("cargo".to_string())
        .arg("elem2".to_string())
        .arg("elem3".to_string());

    assert_eq!(
        BuilderStruct {
            x: 4,
            executable: "cargo".to_string(),
            args: vec!["elem".to_string(), "elem2".to_string(), "elem3".to_string()]
        },
        builder_instance,
        "Expected both the structs to be same"
    );
}
