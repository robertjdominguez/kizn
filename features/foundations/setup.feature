Feature: Foundations at setup
  As a learner starting a topic
  I want kizn to name what it rests on and check whether I have it
  So that I start from solid ground, and decide what to learn underneath

  Background:
    Given a configured author model

  @domain @ui
  Scenario: The draft proposes foundations
    When I start a topic "SPCS" with the goal "run my services on Snowpark Container Services"
    Then the draft proposes 3 foundations, each with why it matters
    When I drop "Networking basics" while shaping it
    Then 2 foundations are kept

  @domain
  Scenario: A foundation I already have becomes a link
    Given a topic I already have called "Linux processes"
    When I start a topic "SPCS" with the goal "run my services on Snowpark Container Services"
    And I confirm it with its foundations
    Then "SPCS" rests on "Linux processes", the topic I have

  @domain @ui
  Scenario: Checking the ground
    Given I confirmed a topic keeping 2 foundations
    Then there are 3 questions on each foundation to check
    When I answer the questions on "Linux processes" mostly wrong
    And I answer the questions on "Union filesystems" mostly right
    Then "Union filesystems" seems solid and "Linux processes" might be shaky
    And kizn prepares "Linux processes" underneath, to start with, and not "Union filesystems"

  @domain
  Scenario: A foundation kizn prepares needs no setting up
    Given I confirmed a topic keeping 2 foundations
    When I answer the questions on "Linux processes" mostly wrong
    Then "Linux processes" is drafted and confirmed by itself, with its first questions written
    And it proposes no foundations of its own

  @domain
  Scenario: Not needing a foundation kizn prepared
    Given I confirmed a topic keeping 2 foundations
    When I answer the questions on "Linux processes" mostly wrong
    And I say I don't need "Linux processes"
    Then "Linux processes" is gone from Today and my map

  @domain
  Scenario: Starting a suggested foundation, in my own words
    Given I confirmed a topic keeping 2 foundations
    And "Networking basics" is suggested under it for later
    When I start "Networking basics" from my map, with a goal of my own
    Then a new topic "Networking basics" is being set up, with my goal
    And the topic I confirmed rests on it
    And its draft is asked to keep to its own terms, as a foundation for the topic I confirmed
