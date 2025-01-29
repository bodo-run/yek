#[cfg(test)]
mod e2e_tests {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn test_empty_dir() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_single_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_multiple_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("test1.txt"), "Test content 1")?;
        fs::write(temp_dir.path().join("test2.txt"), "Test content 2")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_ignore_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("test.log"), "Log content")?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .arg("--ignore-patterns")
            .arg("*.log")
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_priority_rules() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        // Create the 'src' directory
        fs::create_dir(temp_dir.path().join("src"))?;
        fs::write(temp_dir.path().join("src/test.rs"), "Test content")?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let config_content = r#"
            input_dirs = ["."]
            [[priority_rules]]
            pattern = "src/.*\\.rs"
            score = 100
        "#;
        fs::write(temp_dir.path().join("yek.toml"), config_content)?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--config-file")
            .arg(temp_dir.path().join("yek.toml"))
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_binary_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("image.jpg"), [0xFF, 0xD8, 0xFF])?; // Mock JPEG header

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_output_dir() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let output_dir = temp_dir.path().join("output");

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .arg("--output-dir")
            .arg(&output_dir)
            .assert()
            .success();

        assert!(output_dir.exists());
        Ok(())
    }

    #[test]
    fn test_max_size() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .arg("--max-size")
            .arg("1KB")
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_tokens_mode() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .arg("--tokens")
            .arg("100")
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_git_integration() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        // Initialize a Git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()?;

        fs::write(temp_dir.path().join("test.txt"), "Test content")?;
        std::process::Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(temp_dir.path())
            .output()?;
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_multiple_input_dirs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir1 = tempdir()?;
        let temp_dir2 = tempdir()?;
        fs::write(temp_dir1.path().join("test1.txt"), "Test content 1")?;
        fs::write(temp_dir2.path().join("test2.txt"), "Test content 2")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir1.path())
            .arg("--input-dirs")
            .arg(temp_dir2.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_config_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let config_content = r#"
            max_size = "1KB"
            input_dirs = ["."]
        "#;
        fs::write(temp_dir.path().join("yek.toml"), config_content)?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--config-file")
            .arg(temp_dir.path().join("yek.toml"))
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_streaming_mode() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("./repo-serialized/yek-output.txt"));
        Ok(())
    }

    #[test]
    fn test_gitignore_respected() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join(".gitignore"), "*.log")?;
        fs::write(temp_dir.path().join("test.log"), "Log content")?;
        fs::write(temp_dir.path().join("test.txt"), "Test content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_hidden_files_included() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join(".hidden.txt"), "Hidden content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--input-dirs")
            .arg(temp_dir.path())
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_binary_file_extension_config() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("data.bin"), [0, 1, 2, 3])?;

        let config_content = r#"
            input_dirs = ["."]
            binary_extensions = ["bin"]
        "#;
        fs::write(temp_dir.path().join("yek.toml"), config_content)?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--config-file")
            .arg(temp_dir.path().join("yek.toml"))
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_git_boost_config() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let config_content = r#"
            input_dirs = ["."]
            git_boost_max = 50
        "#;
        fs::write(temp_dir.path().join("yek.toml"), config_content)?;

        // Initialize a Git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()?;

        fs::write(temp_dir.path().join("file.txt"), "content")?;
        std::process::Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(temp_dir.path())
            .output()?;
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()?;

        let mut cmd = Command::cargo_bin("yek")?;
        cmd.arg("--config-file")
            .arg(temp_dir.path().join("yek.toml"))
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_default_ignore_license_no_config() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("LICENSE"), "License content")?;

        let mut cmd = Command::cargo_bin("yek")?;
        let output = cmd.arg(temp_dir.path()).output()?;

        // Assert that the command was successful
        assert!(output.status.success());

        // Convert stdout bytes to a string
        let stdout = String::from_utf8(output.stdout)?;

        // Assert that the output does not contain "License content"
        assert!(
            !stdout.contains("License content"),
            "Output should not contain 'License content'"
        );

        Ok(())
    }

    #[test]
    fn test_default_ignore_license_empty_config() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("LICENSE"), "License content")?;
        fs::write(
            temp_dir.path().join("yek.yaml"),
            "ignore_patterns: []\n", // Empty ignore_patterns
        )?;

        let mut cmd = Command::cargo_bin("yek")?;
        let output = cmd
            .arg("--config-file")
            .arg(temp_dir.path().join("yek.yaml"))
            .arg(temp_dir.path())
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;
        assert!(
            !stdout.contains("License content"),
            "Output should not contain 'License content' even with empty config"
        );

        Ok(())
    }

    #[test]
    fn test_gitignore_whitelist_license() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("LICENSE"), "License content")?;
        fs::write(temp_dir.path().join(".gitignore"), "!LICENSE\n")?;

        let mut cmd = Command::cargo_bin("yek")?;
        let output = cmd.arg(temp_dir.path()).output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;

        assert!(
            stdout.contains("License content"),
            "Output should contain 'License content' because .gitignore whitelists it"
        );

        Ok(())
    }

    #[test]
    fn test_windows_path_normalization() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("LICENSE"), "License content")?;
        // TODO:
        // Use a path with mixed slashes to simulate potential Windows issues
        // let windows_path = format!(
        //     "{}\\LICENSE",
        //     temp_dir.path().to_string_lossy().replace("/", "\\")
        // );

        let mut cmd = Command::cargo_bin("yek")?;
        let output = cmd.arg(temp_dir.path()).output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout)?;

        assert!(
            !stdout.contains("License content"),
            "Output should not contain 'License content' even with Windows-style paths"
        );

        Ok(())
    }
}
