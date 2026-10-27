Feature: When I'm stuck
  As a learner who just missed a card and doesn't see why
  I want to ask, look again, and read up, without losing my place
  So that I understand it before it comes back

  Background:
    Given a configured author model

  @ui
  Scenario: From a missed card to understanding it
    Given I missed a card
    When I ask about it "What's wrong with my pick?"
    And I close the tutor
    And I say "This key is wrong"
    And I keep the question
    And I finish the rest of the set
    And I open what I missed
    And I pick a passage in the page and ask "Explain this"
    And I close the tutor and the reader, and mark it studied
    Then Today says I'm all caught up
