Feature: Letting a topic go
  As a learner whose plans change
  I want to delete a topic, or a draft I won't finish
  So that Today and my map show only what I'm learning

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Deleting a topic
    Given topics "SPCS", "Container fundamentals" and "Linux processes"
    And "SPCS" rests on "Container fundamentals", which rests on "Linux processes"
    When I delete "Container fundamentals"
    Then "Container fundamentals" is gone from Today and my map
    And "SPCS" and "Linux processes" are still there

  @domain @ui
  Scenario: Discarding a draft
    Given a drafted topic
    When I discard the draft
    Then "Container fundamentals" is gone from Today and my map
