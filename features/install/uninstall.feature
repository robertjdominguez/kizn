Feature: Uninstall
  As someone who installed kizn with the script
  I want to remove it without losing my learning by accident
  So that only I decide when my data goes

  Background:
    Given a staged release of kizn v0.1.0
    And kizn was installed with the script
    And I have kizn data

  @install @linux @macos @windows
  Scenario: Uninstalling keeps my data
    When I run the install script with --uninstall
    Then kizn is no longer installed
    And kizn no longer appears in my launcher
    And my kizn data is still there

  @install @linux @macos @windows
  Scenario: Purging removes my data too
    When I run the install script with --uninstall --purge
    Then kizn is no longer installed
    And my kizn data is gone
