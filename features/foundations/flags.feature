Feature: Flags from my answers
  As a learner whose misses point at something more basic
  I want kizn to say so, and let me decide
  So that I fix the real gap rather than drilling the symptom

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Misses that point at a foundation
    Given I missed 2 questions that point at "Union filesystems"
    Then the topic and Today flag "Union filesystems"
    When I add it underneath
    Then the flag is gone and "Union filesystems" is suggested on my map

  @domain
  Scenario: Not now stays gone
    Given I missed 2 questions that point at "Union filesystems"
    When I say "not now" to the flag
    Then the flag is gone, after a restart too

  @domain
  Scenario: Misses that point at a topic I already rest on
    Given I missed 2 questions that point at "Union filesystems"
    And "Union filesystems" is a topic my topic already rests on
    Then the flag says it's worth a review
