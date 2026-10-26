Feature: First practice
  As a learner whose topic has questions
  I want to answer a few, see why each option is right or wrong, and see where I stand
  So that I know what I already know and what to do next

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Placement on new concepts
    Given a topic whose bank has recall questions for 5 unseen concepts
    When I start practice
    Then I get 5 recall cards, one per concept
    And the correct option's position varies between cards

  @domain @ui
  Scenario: Feedback on a tap
    Given I'm practising a topic
    When I tap a wrong option
    Then I see that it is wrong, every option's why, and the source
    And the concept is "seen"

  @domain @ui
  Scenario: Summary
    Given I'm practising a topic
    When I finish the set
    Then I see what held, what slipped, the next due date
    And each outcome's count and what's next for it

  @domain
  Scenario: Stopping halfway keeps what I answered
    Given I'm practising a topic
    When I answer 2 cards and stop
    Then those 2 answers count
    And the summary covers the 2 cards I answered

  @domain
  Scenario: Practice never calls a model
    Given a topic whose bank has recall questions for 5 unseen concepts
    And the author model can't be reached
    When I start practice and answer every card
    Then nothing waited on a model

  @domain
  Scenario: The first set is ready long before the whole bank
    When I confirm a topic
    Then practice is available once every concept on the first outcome has one question
    And the rest of the bank is still being written

  @domain @ui
  Scenario: Today says what to do next
    Given I finished my first set today
    When I open Today
    Then I see when my next recalls are due
