Feature: Read in kizn
  As a learner studying a concept
  I want to read or watch its resources inside kizn, with my notes beside them
  So that I can learn it properly, and the tutor can discuss what I'm reading

  Background:
    Given a configured author model

  @domain @ui
  Scenario: Reading a page
    Given a concept with a verified page
    When I open it in kizn
    Then I read its text with the concept on top
    And not the page's scripts or navigation

  @domain @ui
  Scenario: A video is offered with its transcript
    Given I missed a concept in my starting point
    When kizn looks for resources for it
    Then a video with captions is offered
    And opening it shows its transcript with timestamps

  @domain
  Scenario: A video without a transcript isn't offered
    Given I missed a concept in my starting point
    When kizn looks for resources for it
    Then the video without captions is recorded as having no transcript

  @domain @ui
  Scenario: Notes stay with the concept
    Given a concept with a verified page
    When I write a note while reading it
    And I come back to the concept later
    Then the note is there

  @domain
  Scenario: Adding my own source
    Given a concept with a verified page
    When I add "https://notes.example.com/web/article/an-image-is-immutable-layers-plus-config" to it
    Then kizn fetches it once and offers it

  @domain
  Scenario: A source kizn can't read
    Given a concept with a verified page
    When I add "https://app.example.com/web/shell" to it
    Then I'm told it couldn't be read, and why
