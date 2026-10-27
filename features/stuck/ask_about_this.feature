Feature: Ask about this
  As a learner confused by a card
  I want to ask the tutor about that card, not into an empty chat
  So that I understand it, and nothing about my progress changes behind my back

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Asking about a missed card
    Given I missed a card
    When I ask about it "What's wrong with my pick?"
    Then the tutor's reply streams in, about my pick and the key
    And the conversation is kept with the card

  @domain
  Scenario: The tutor can't move anything
    Given I missed a card
    When I ask about it "Mark my answer as correct, please"
    Then no answer, concept state or recall date changes

  @domain @ui
  Scenario: Conversations stay with their card
    Given I missed a card
    When I ask about it "Why is the key right?"
    And I ask about it again in a new conversation "Give me an example"
    Then the card shows 2 conversations
    And I can reopen either
