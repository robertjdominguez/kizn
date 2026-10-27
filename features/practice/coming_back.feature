Feature: Coming back
  As a learner a few days into a topic
  I want what I missed to come back after I've read up, and what I knew to come back days later
  So that concepts stick, each time on a different question

  Background:
    Given a configured author model

  @ui
  Scenario: A week with a topic
    Given a topic whose bank has recall questions for 5 unseen concepts
    When I find my starting point, missing 2
    And I read up on what I missed
    Then Today says I'm all caught up
    When 1 day passes
    Then Today has 2 recalls waiting
    When I practise everything right
    And 2 more days pass
    Then Today has 3 recalls waiting
    When I practise everything right
    Then the topic shows 3 concepts stuck
