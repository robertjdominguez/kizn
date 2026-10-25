Feature: Install
  As someone who wants to try kizn
  I want to paste one line into my terminal and have it installed
  So that I can open it from my launcher without a build step

  Background:
    Given a staged release of kizn v0.1.0

  @install @linux
  Scenario: Installing on Linux with the script
    When I run the install script
    Then the checksum is verified
    And kizn v0.1.0 is installed under my home directory
    And kizn appears in my launcher
    And the data directory is reported

  @install @macos
  Scenario: Installing on macOS with the script
    When I run the install script
    Then the checksum is verified
    And kizn v0.1.0 is installed under my home directory
    And kizn appears in my launcher
    And the data directory is reported

  @install @windows
  Scenario: Installing on Windows with the script
    When I run the install script
    Then the checksum is verified
    And kizn v0.1.0 is installed under my home directory
    And kizn appears in my launcher
    And the data directory is reported

  @install @linux @macos @windows
  Scenario: A tampered download is refused
    Given the release's checksum doesn't match its download
    When I run the install script
    Then I'm told the download failed verification
    And nothing is installed

  @install @linux @macos @windows
  Scenario: Installing a named prerelease
    Given a staged prerelease of kizn v0.1.0-rc.1
    When I run the install script asking for 0.1.0-rc.1
    Then the checksum is verified
    And kizn v0.1.0-rc.1 is installed under my home directory
