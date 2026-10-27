Feature: Learn what you missed
  As a learner who missed concepts in my starting point
  I want sources kizn has checked, so I can learn them before they come back
  So that the next recall is about remembering, not guessing again

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Verified sources for a missed concept
    Given I missed a concept in my starting point
    When I open it
    Then I see the sources kizn fetched and could read
    And none it couldn't

  @domain
  Scenario: A suggested page that's an empty app shell isn't offered
    Given I missed a concept in my starting point
    When I open it
    Then the app shell and the dead link are recorded with their reasons

  @domain @ui
  Scenario: Marking a concept studied
    Given I missed a concept in my starting point
    When I open it and mark it studied
    Then the summary shows it studied
    And it still comes back as recall tomorrow

  @domain
  Scenario: Studying is a suggestion, never a gate
    Given I missed a concept in my starting point
    When a day passes without studying it
    Then it's in the practice set anyway

  @domain @ui
  Scenario: Today offers the reading when nothing is due
    Given I missed a concept in my starting point
    And I answer the rest of the set right
    When I open Today after the set
    Then Today's next step is to read up on 1 concept
    And this week shows today as practised
