Feature: This key is wrong
  As a learner who thinks a question's key is wrong
  I want to say so after a moment to look again
  So that the answer stops counting and the question never comes back

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Disputing voids the answer
    Given I missed a card
    When I say "This key is wrong" and confirm
    Then that answer moves nothing
    And the question never appears again
    And a replacement is written for its slot

  @ui
  Scenario: A moment to look again
    Given I missed a card
    When I say "This key is wrong"
    Then I see kizn's reason for its key first
    When I keep the question
    Then my answer still counts
