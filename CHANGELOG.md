# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

### Changed

## [0.6.0] - 2024-01-15

- Simplified terminal output handling
- Improved terminal output handling with stderr for progress updates

### Changed

- Enhanced user experience with visual progress feedback
- Better handling of terminal output in different environments

## [0.5.0] - 2024-01-15

### Changed

- Changed priority ordering to put high priority files last for better LLM context
- Updated tests to verify new priority ordering behavior

## [0.4.0] - 2024-01-13

### Added

- Smarter test file detection
- Enhanced file processing performance
- Improved error handling and reporting

### Changed

- Updated dependencies to latest versions
- Refined documentation and examples

## [0.3.0] - 2024-01-13

### Added

- Comprehensive test suite covering all major functionality
- Integration tests for file handling, ignore patterns, and priorities
- Debug output for better visibility into file processing

### Fixed

- File priority handling now correctly sorts files by priority score
- Validation error messages now properly output to stderr
- Binary file detection and handling improvements
- Gitignore pattern handling fixes

### Changed

- Improved file processing to collect and sort before processing
- Enhanced error handling and validation messages
- Better debug logging throughout the codebase

## [0.2.0] - 2024-01-11

### Added

- Initial release with basic functionality
- Support for processing repository files
- Configuration via yek.toml
- Priority rules for file processing
- Ignore patterns support
- Binary file detection
