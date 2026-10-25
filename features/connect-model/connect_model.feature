Feature: Connect a model
  As a learner bringing my own model
  I want kizn to check the model works before it relies on it
  So that I find out now, in plain words, not halfway through a topic

  @domain @ui
  Scenario: Connecting an OpenAI-compatible local model
    Given no model is configured
    When I add a provider "local" with protocol "openai" at "http://gpu:11434/v1" and model "qwen3:32b"
    And I test the connection
    Then I see "works"
    And all three roles use "local"

  @domain @ui
  Scenario: A model that can't produce structured output
    Given a provider whose replies are not valid JSON
    When I test the connection
    Then I see that it can tutor but can't author items
    And I can't assign it the author role

  @domain
  Scenario: The key is stored in the keychain, not the config
    When I add a provider "claude" with protocol "anthropic" and a key
    Then config.toml references the key by name only

  @domain @ui
  Scenario: Nothing answers at the address
    Given a provider at an address where nothing is listening
    When I test the connection
    Then I see that nothing answered at that address
    And no role can be assigned to it

  @domain
  Scenario: A model without schema-constrained output still authors
    Given a provider that ignores JSON schemas but answers in JSON when asked
    When I test the connection
    Then I see that it works and kizn will parse its replies
    And I can assign it the author role

  @domain @ui
  Scenario: Editing config.toml by hand
    Given kizn is open on Settings
    When I add a provider to config.toml by hand
    Then Settings shows it without a restart

  @domain
  Scenario: An account that can't pay for requests
    Given a provider whose account has no credit
    When I test the connection
    Then I see that the account can't run requests yet, in the provider's own words
    And no role can be assigned to it
