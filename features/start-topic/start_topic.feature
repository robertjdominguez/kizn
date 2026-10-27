Feature: Start a topic
  As a learner with something to learn for work
  I want to say what I need to be able to do and why
  So that kizn proposes what to learn, I shape it, and practice gets prepared

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Drafting outcomes from a goal
    When I start a topic "Container fundamentals" with the goal "explain to customers why deployments behave differently"
    Then I see 3 to 5 drafted outcomes, each starting with a verb
    And each outcome has a target with a reason that quotes my goal
    And each outcome has 3 to 7 concepts

  @domain @ui
  Scenario: Editing before confirming
    Given a drafted topic
    When I rename a concept, delete an outcome and change a target to "judge and recommend"
    And I confirm the topic
    Then bank writing is queued for each outcome
    And the topic page says its questions are being written

  @domain
  Scenario: An item that fails a check is never shown
    Given the author writes an item whose correct option is the longest by far
    When the bank is written
    Then that item is refused with the reason "longest-option cue"
    And it never becomes active

  @domain
  Scenario: The blind check gates activation
    Given the checker picks a different option than the key
    When the bank is written
    Then the item is not active

  @domain @ui
  Scenario: Today follows a topic while practice is prepared
    Given a confirmed topic whose bank is being written
    When I open Today
    Then I see the topic as preparing
    When every outcome has items
    Then I see the topic as ready

  @domain
  Scenario: Writing fails and is retried
    Given a confirmed topic whose author account has no credit
    When the bank is written
    Then each outcome says the account can't run requests yet
    When I retry an outcome
    Then kizn tries writing it again straight away

  @domain
  Scenario: Questions that keep failing the checks rest, then come back by themselves
    Given a confirmed topic whose checker refuses every question
    When the bank is written
    Then each outcome rests, and kizn will try it again by itself

  @domain
  Scenario: A draft that fails can carry on
    Given drafting failed because the author couldn't run requests
    When I fix the author and try again
    Then drafting carries on and every outcome gets its concepts
