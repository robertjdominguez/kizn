Feature: The map
  As a learner with several topics
  I want to see them as a map of what rests on what
  So that I can tell what's foundational and what's waiting underneath

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Layers and suggestions
    Given topics "SPCS", "Container fundamentals" and "Linux processes"
    And "SPCS" rests on "Container fundamentals", which rests on "Linux processes"
    And "Networking basics" is suggested under "SPCS"
    When I open the map
    Then "Linux processes" is at the bottom and "SPCS" at the top
    And "Networking basics" is drawn as a suggestion under "SPCS"
