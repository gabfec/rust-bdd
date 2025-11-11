
Feature: Ping Pong

  Scenario: Test basic communication
    Given I run broker
    When I send message PingRequest
    Then I expect message PongReply
      """
      {
        "message": "Hello"
      }
      """

