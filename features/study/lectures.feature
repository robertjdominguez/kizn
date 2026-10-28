Feature: A lecture for each concept
  As a learner who missed something
  I want it explained in depth, every part backed by a source I can open
  So that I understand it, and can trust what I read

  Background:
    Given a configured author model

  @domain @ui
  Scenario: A lecture for a concept I missed
    Given I missed a concept in my starting point
    When I open it and its lecture is written
    Then every paragraph, key term and mix-up cites the sources kizn read
    And it takes on the mix-ups my wrong answers showed
    And none of it repeats a question's wording

  @domain
  Scenario: What its sources don't back is left out
    Given a checker that can't back a lecture's first paragraph
    And I missed a concept in my starting point
    When I open it and its lecture is written
    Then that paragraph isn't in it, and the rest still cites its sources

  @domain
  Scenario: Written when it's needed
    Given I'm practising a topic
    Then no lecture is written for concepts I haven't missed or opened
