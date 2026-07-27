# Language Tutor

A web-application for a french tutor to converse with and learn grammar along.

The project should be implemented in RUST and using ollama with the selected model. THe project should be light-weight because it will only serve one user and interact with ollama using rest API.

## Configuration File

There is a yaml-configuration file [configuration-file](./config.yaml)

```yaml
model: gemma-4:31
TUTOR: Amelie
TUTOR_GENDER: female
USER: Thorsten
EXPLANATION_LANGUAGE: German
LEARNING_LANGUAGE: French
```

and there is a primer template stored in [tutor.md](./tutor.md). The fields of {{}} should be filled with the data from the config-file and the user entries.

config-file:

- TUTOR
- TUTOR_GENDER
- USER
- EXPLANATION_LANGUAGE
- LEARNING_LANGUAGE

Configured in app:

- SKILL_LEVEL
- TOPIC
- GRAMMAR_FOCUS

## UI

At the top there is the title:
Greeting of the {{USER}} in the LEARNING_LANGUAGE by the {{TUTOR}}" using the parameters from the config-file.

The main panel should consist of these fields:

1. text field for adding a topic: TOPIC
2. pull down menue for selecting a skill levels from A1 to C1: SKILL_LEVEL
3. pull down menue for selecting the grammar to focus on: GRAMMAR_FOCUS
4. panel for the dialog.

The conversion should be stored to the folder "dialogues" according to <topic>-<grammar>-<skill-level>-<timestamp>.
The grammar topics are stored to an md-file french-grammar.md. The subtitles defines the grammar topic and the text gives a more detailed description of this grammar section. The menue for the grammar should list the grammar topics.
