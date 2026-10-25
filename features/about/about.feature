Feature: About
  As someone running kizn
  I want to see which version I have and where my data lives
  So that I can report problems and back up the right folder

  @domain @ui
  Scenario: About shows the version and where data lives
    Given a fresh data directory
    When I open kizn
    And I open About
    Then I see the version of kizn I'm running
    And I see where my data is kept
