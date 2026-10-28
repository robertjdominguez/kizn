Feature: Sources for a concept
  As a learner studying a concept
  I want the sources kizn read for it, to explore further, and my notes kept with it
  So that its lecture rests on sources I can open, and the tutor can draw on them

  Background:
    Given a configured author model

  @domain @ui
  Scenario: A video is offered with its transcript
    Given I missed a concept in my starting point
    When kizn looks for resources for it
    Then a video with captions is offered

  @domain
  Scenario: A video without a transcript isn't offered
    Given I missed a concept in my starting point
    When kizn looks for resources for it
    Then the video without captions is recorded as having no transcript

  @domain @ui
  Scenario: Notes stay with the concept
    Given a concept with a verified page
    When I write a note while reading its lecture
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

  @domain
  Scenario: Questions cite the resources
    Given a concept with a verified page
    When a question about it is replaced
    Then the new question cites one of its resources
