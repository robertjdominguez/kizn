Feature: First run
  As someone who has just installed kizn
  I want to land somewhere that tells me what to do first

  @domain @ui @smoke
  Scenario: A new install opens on an empty Today
    Given a fresh data directory
    When I open kizn
    Then I see "Start a topic"
    And no model is configured
