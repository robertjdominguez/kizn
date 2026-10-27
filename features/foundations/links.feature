Feature: Topics that rest on topics
  As a learner with several topics
  I want to say which ones rest on which
  So that I see what's foundational, at any depth, and kizn never draws a loop

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Making one topic a foundation of another
    Given topics "SPCS" and "Container fundamentals"
    When I make "Container fundamentals" a foundation of "SPCS"
    Then "SPCS" rests on "Container fundamentals"
    And "Container fundamentals" supports "SPCS"

  @domain
  Scenario: Foundations of foundations
    Given topics "SPCS", "Container fundamentals" and "Linux processes"
    When I make "Container fundamentals" a foundation of "SPCS"
    And I make "Linux processes" a foundation of "Container fundamentals"
    Then "SPCS" sits two layers above "Linux processes"

  @domain @ui
  Scenario: A loop is refused
    Given topics "SPCS", "Container fundamentals" and "Linux processes"
    And "SPCS" rests on "Container fundamentals", which rests on "Linux processes"
    When I make "SPCS" a foundation of "Linux processes"
    Then I'm told it would make a loop

  @domain
  Scenario: Part of a broader topic
    Given topics "SPCS" and "SPCS networking"
    When I make "SPCS networking" part of "SPCS"
    Then "SPCS" is made up of "SPCS networking"
