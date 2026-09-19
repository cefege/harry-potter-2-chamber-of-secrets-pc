import Quickshell
import Quickshell.Io
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ShellRoot {
	id: root
	property string requestPath: Quickshell.env("HP2_LAUNCHER_REQUEST")
	property string resultPath: Quickshell.env("HP2_LAUNCHER_RESULT")
	property var req: null
	property var settings: ({})
	property int selectedSaveIndex: -1
	property bool isProcessing: false
	property bool loadFailed: false

	FileView {
		id: requestView
		path: root.requestPath
		onLoaded: {
			root.req = JSON.parse(text())
			// Shallow-copy settings into a plain JS object so bindings react to reassignment.
			root.settings = Object.assign({}, root.req.settings)
		}
		onLoadFailed: (err) => {
			root.loadFailed = true
		}
	}

	FileView {
		id: resultWriter
		path: root.resultPath
		onSaved: Qt.quit()
		onSaveFailed: (err) => {
			console.log("hp2-launcher: failed to write result: " + err)
			Qt.quit()
		}
	}

	function commitAndExit(action, extra) {
		if (root.isProcessing) return
		root.isProcessing = true

		let lines = ["action=" + action]
		if (extra) {
			for (const key in extra)
				lines.push(key + "=" + extra[key])
		}
		if (root.req) {
			const s = root.settings
			lines.push("settings.screenMode=" + s.screenMode)
			lines.push("settings.renderBackend=" + s.renderBackend)
			lines.push("settings.textureDetail=" + s.textureDetail)
			lines.push("settings.objectDetail=" + s.objectDetail)
			lines.push("settings.difficulty=" + s.difficulty)
			lines.push("settings.controlMode=" + s.controlMode)
			lines.push("settings.resolutionWidth=" + s.resolutionWidth)
			lines.push("settings.resolutionHeight=" + s.resolutionHeight)
			lines.push("settings.verticalSync=" + (s.verticalSync ? "true" : "false"))
			lines.push("settings.renderScale=" + s.renderScale)
			lines.push("settings.uiScale=" + s.uiScale)
			lines.push("settings.frameRateLimit=" + s.frameRateLimit)
			lines.push("settings.showFPS=" + (s.showFPS ? "true" : "false"))
			lines.push("settings.maintainVerticalFOV=" + (s.maintainVerticalFOV ? "true" : "false"))
			lines.push("settings.nativeText=" + (s.nativeText ? "true" : "false"))
			lines.push("settings.antiAliasingSamples=" + s.antiAliasingSamples)
			lines.push("settings.anisotropy=" + s.anisotropy)
			lines.push("settings.brightness=" + s.brightness)
			lines.push("settings.soundEnabled=" + (s.soundEnabled ? "true" : "false"))
			lines.push("settings.soundVolume=" + s.soundVolume)
			lines.push("settings.musicVolume=" + s.musicVolume)
			lines.push("settings.mouseSensitivity=" + s.mouseSensitivity)
			lines.push("settings.invertMouse=" + (s.invertMouse ? "true" : "false"))
			lines.push("settings.autoCenterCamera=" + (s.autoCenterCamera ? "true" : "false"))
			lines.push("settings.moveWhileCasting=" + (s.moveWhileCasting ? "true" : "false"))
			lines.push("settings.autoQuaff=" + (s.autoQuaff ? "true" : "false"))
			lines.push("settings.screenFlashes=" + (s.screenFlashes ? "true" : "false"))
			lines.push("settings.joystickEnabled=" + (s.joystickEnabled ? "true" : "false"))

			// No data-source switcher in this UI yet; echo the configuration
			// back unchanged so the engine's persisted choice survives.
			lines.push("dataSources.selected=" + root.req.dataSources.selected)
			lines.push("dataSources.retailRoot=" + root.req.dataSources.retailRoot)
			lines.push("dataSources.prototypeRoot=" + root.req.dataSources.prototypeRoot)
		}

		resultWriter.setText(lines.join("\n") + "\n")
	}

	FloatingWindow {
		id: window
		title: "Harry Potter and the Chamber of Secrets"
		implicitWidth: 1000
		implicitHeight: 700
		minimumSize: Qt.size(1000, 700)
		visible: true
		color: "#13141c"

		ColumnLayout {
			anchors.fill: parent
			anchors.margins: 20
			spacing: 16

			Rectangle {
				Layout.fillWidth: true
				height: 70
				color: "#24283b"
				radius: 8
				border.color: "#414868"
				border.width: 1

				ColumnLayout {
					anchors.fill: parent
					anchors.margins: 15
					spacing: 4

					Text {
						text: "Harry Potter 2"
						font.pixelSize: 22
						font.bold: true
						color: "#eb927b"
					}
					Text {
						text: root.loadFailed ? "Failed to load launcher request" : "Chamber of Secrets"
						font.pixelSize: 14
						color: root.loadFailed ? "#f7768e" : "#565f89"
					}
				}
			}

			Text {
				visible: root.req && root.req.errorMessage && root.req.errorMessage.length > 0
				text: root.req ? root.req.errorMessage : ""
				color: "#f7768e"
				wrapMode: Text.WordWrap
				Layout.fillWidth: true
			}

			RowLayout {
				Layout.fillWidth: true
				Layout.fillHeight: true
				spacing: 20

				// Left: Saves
				ColumnLayout {
					Layout.preferredWidth: 230
					Layout.minimumWidth: 200
					Layout.maximumWidth: 260
					Layout.fillHeight: true
					spacing: 10

					Text {
						text: "SAVED GAMES"
						font.pixelSize: 12
						font.bold: true
						color: "#565f89"
						font.capitalization: Font.AllUppercase
						font.letterSpacing: 1.5
					}

					Rectangle {
						Layout.fillWidth: true
						Layout.fillHeight: true
						color: "#13141c"
						radius: 6
						border.color: "#414868"
						border.width: 1

						ListView {
							anchors.fill: parent
							anchors.margins: 1
							model: root.req ? root.req.saves : []
							clip: true
							spacing: 1

							delegate: Rectangle {
								width: parent ? parent.width : 0
								height: 60
								color: root.selectedSaveIndex === index ? "#7aa2f7" : "#0e0e14"
								border.color: root.selectedSaveIndex === index ? "#7da6ff" : "#414868"
								border.width: 1

								MouseArea {
									anchors.fill: parent
									onClicked: root.selectedSaveIndex = index
								}

								ColumnLayout {
									anchors.fill: parent
									anchors.margins: 10
									spacing: 3

									Text {
										text: modelData.displayName || ("Slot " + modelData.slot)
										font.pixelSize: 13
										font.bold: true
										color: root.selectedSaveIndex === index ? "#c0caf5" : "#a9b1d6"
										Layout.fillWidth: true
										elide: Text.ElideRight
									}

									RowLayout {
										spacing: 12
										Text {
											text: new Date(modelData.modifiedSeconds * 1000).toLocaleString()
											font.pixelSize: 11
											color: root.selectedSaveIndex === index ? "#b4bee6" : "#565f89"
										}
										Text {
											text: (modelData.size / (1024*1024)).toFixed(1) + " MB"
											font.pixelSize: 11
											color: root.selectedSaveIndex === index ? "#b4bee6" : "#565f89"
										}
									}
								}
							}
						}
					}
				}

				// Right: Settings
				ColumnLayout {
					Layout.fillWidth: true
					Layout.fillHeight: true
					spacing: 10

					Text {
						text: "SETTINGS"
						font.pixelSize: 12
						font.bold: true
						color: "#565f89"
						font.capitalization: Font.AllUppercase
						font.letterSpacing: 1.5
					}

					TabBar {
						id: tabBar
						Layout.fillWidth: true
						background: Rectangle {
							color: "#24283b"
							radius: 6
							border.color: "#414868"
							border.width: 1
						}

						TabButton {
							text: "Display"
							background: Rectangle { color: tabBar.currentIndex === 0 ? "#7aa2f7" : "transparent"; radius: 4 }
							contentItem: Text { text: parent.text; color: tabBar.currentIndex === 0 ? "#c0caf5" : "#565f89"; font.bold: true; font.pixelSize: 11; horizontalAlignment: Text.AlignHCenter }
						}
						TabButton {
							text: "Graphics"
							background: Rectangle { color: tabBar.currentIndex === 1 ? "#7aa2f7" : "transparent"; radius: 4 }
							contentItem: Text { text: parent.text; color: tabBar.currentIndex === 1 ? "#c0caf5" : "#565f89"; font.bold: true; font.pixelSize: 11; horizontalAlignment: Text.AlignHCenter }
						}
						TabButton {
							text: "Audio"
							background: Rectangle { color: tabBar.currentIndex === 2 ? "#7aa2f7" : "transparent"; radius: 4 }
							contentItem: Text { text: parent.text; color: tabBar.currentIndex === 2 ? "#c0caf5" : "#565f89"; font.bold: true; font.pixelSize: 11; horizontalAlignment: Text.AlignHCenter }
						}
						TabButton {
							text: "Gameplay"
							background: Rectangle { color: tabBar.currentIndex === 3 ? "#7aa2f7" : "transparent"; radius: 4 }
							contentItem: Text { text: parent.text; color: tabBar.currentIndex === 3 ? "#c0caf5" : "#565f89"; font.bold: true; font.pixelSize: 11; horizontalAlignment: Text.AlignHCenter }
						}
					}

					Rectangle {
						Layout.fillWidth: true
						Layout.fillHeight: true
						color: "#24283b"
						radius: 6
						border.color: "#414868"
						border.width: 1

						StackLayout {
							anchors.fill: parent
							anchors.margins: 16
							currentIndex: tabBar.currentIndex
							clip: true

							// --- Display ---
							ScrollView {
								contentWidth: availableWidth
								ColumnLayout {
									width: parent.width
									spacing: 14

									RowLayout {
										spacing: 12
										Text { text: "Render Backend:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: ["XOpenGL", "Vulkan"]
											currentIndex: root.settings.renderBackend === "vulkan" ? 1 : 0
											onActivated: (idx) => root.settings.renderBackend = idx === 1 ? "vulkan" : "xopengl"
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Native Text Rendering:"; color: "#565f89"; Layout.minimumWidth: 130 }
										CheckBox {
											checked: !!root.settings.nativeText
											onToggled: root.settings.nativeText = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Screen Mode:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: ["Windowed", "Fullscreen", "Borderless"]
											currentIndex: {
												const m = root.settings.screenMode
												return m === "fullscreen" ? 1 : m === "borderlessDesktop" ? 2 : 0
											}
											onActivated: (idx) => {
												root.settings.screenMode = idx === 1 ? "fullscreen" : idx === 2 ? "borderlessDesktop" : "windowed"
											}
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Resolution:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: root.req ? root.req.displayModes.map(m => m.label) : []
											currentIndex: {
												if (!root.req) return 0
												for (let i = 0; i < root.req.displayModes.length; i++) {
													if (root.req.displayModes[i].width === root.settings.resolutionWidth &&
														root.req.displayModes[i].height === root.settings.resolutionHeight)
														return i
												}
												return 0
											}
											onActivated: (idx) => {
												if (root.req && root.req.displayModes[idx]) {
													root.settings.resolutionWidth = root.req.displayModes[idx].width
													root.settings.resolutionHeight = root.req.displayModes[idx].height
												}
											}
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "V-Sync:"; color: "#565f89"; Layout.minimumWidth: 130 }
										CheckBox {
											checked: !!root.settings.verticalSync
											onToggled: root.settings.verticalSync = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Frame Rate Limit:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: root.req ? root.req.choices.frameRateLimit.map(v => v === 0 ? "Unlimited" : String(v)) : ["Unlimited"]
											currentIndex: root.req ? root.req.choices.frameRateLimit.indexOf(root.settings.frameRateLimit) : 0
											onActivated: (idx) => { if (root.req) root.settings.frameRateLimit = root.req.choices.frameRateLimit[idx] }
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Show FPS:"; color: "#565f89"; Layout.minimumWidth: 130 }
										CheckBox {
											checked: !!root.settings.showFPS
											onToggled: root.settings.showFPS = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}


									RowLayout {
										spacing: 12
										Text { text: "Widescreen View:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: ["Preserve Vertical View", "Original 4:3 Framing"]
											currentIndex: root.settings.maintainVerticalFOV ? 0 : 1
											onActivated: (idx) => root.settings.maintainVerticalFOV = (idx === 0)
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									Item { Layout.fillHeight: true }
								}
							}

							// --- Graphics ---
							ScrollView {
								contentWidth: availableWidth
								ColumnLayout {
									width: parent.width
									spacing: 14

									RowLayout {
										spacing: 12
										Text { text: "Texture Detail:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: ["Low", "Medium", "High"]
											currentIndex: ["low", "medium", "high"].indexOf(root.settings.textureDetail)
											onActivated: (idx) => root.settings.textureDetail = ["low", "medium", "high"][idx]
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Object Detail:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: ["Very Low", "Low", "Medium", "High", "Very High"]
											currentIndex: ["veryLow", "low", "medium", "high", "veryHigh"].indexOf(root.settings.objectDetail)
											onActivated: (idx) => root.settings.objectDetail = ["veryLow", "low", "medium", "high", "veryHigh"][idx]
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Anti-Aliasing:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: root.req ? root.req.choices.antiAliasingSamples.map(v => v === 0 ? "Off" : v + "x") : ["Off"]
											currentIndex: root.req ? root.req.choices.antiAliasingSamples.indexOf(root.settings.antiAliasingSamples) : 0
											onActivated: (idx) => { if (root.req) root.settings.antiAliasingSamples = root.req.choices.antiAliasingSamples[idx] }
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Texture Filtering:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: root.req ? root.req.choices.anisotropy.map(v => v === 0 ? "Off" : v + "x") : ["Off"]
											currentIndex: root.req ? root.req.choices.anisotropy.indexOf(root.settings.anisotropy) : 0
											onActivated: (idx) => { if (root.req) root.settings.anisotropy = root.req.choices.anisotropy[idx] }
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Render Scale:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: root.req ? root.req.choices.renderScale.map(v => Math.round(v * 100) + "%") : ["100%"]
											currentIndex: root.req ? root.req.choices.renderScale.indexOf(root.settings.renderScale) : 0
											onActivated: (idx) => { if (root.req) root.settings.renderScale = root.req.choices.renderScale[idx] }
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "UI Scale:"; color: "#565f89"; Layout.minimumWidth: 130 }
										ComboBox {
											Layout.fillWidth: true
											model: root.req ? root.req.choices.uiScale.map(v => Math.round(v * 100) + "%") : ["100%"]
											currentIndex: root.req ? root.req.choices.uiScale.indexOf(root.settings.uiScale) : 0
											onActivated: (idx) => { if (root.req) root.settings.uiScale = root.req.choices.uiScale[idx] }
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Brightness:"; color: "#565f89"; Layout.minimumWidth: 130 }
										Text { text: "10%"; color: "#565f89"; font.pixelSize: 10 }
										Slider {
											Layout.fillWidth: true
											from: 0.1; to: 1.0
											value: root.settings.brightness !== undefined ? root.settings.brightness : 0.4
											onMoved: root.settings.brightness = value
											background: Rectangle { height: 4; color: "#414868"; radius: 2
												Rectangle { height: parent.height; width: parent.width * parent.parent.position; color: "#7aa2f7"; radius: 2 }
											}
											handle: Rectangle {
												x: parent.leftPadding + parent.availableWidth * parent.visualPosition - width/2
												y: parent.topPadding + parent.availableHeight/2 - height/2
												width: 14; height: 14; radius: 7; color: "#7da6ff"; border.color: "#414868"; border.width: 1
											}
										}
										Text { text: "100%"; color: "#565f89"; font.pixelSize: 10 }
									}

									Item { Layout.fillHeight: true }
								}
							}

							// --- Audio ---
							ScrollView {
								contentWidth: availableWidth
								ColumnLayout {
									width: parent.width
									spacing: 14

									RowLayout {
										spacing: 12
										Text { text: "Enable Sound:"; color: "#565f89"; Layout.minimumWidth: 130 }
										CheckBox {
											checked: !!root.settings.soundEnabled
											onToggled: root.settings.soundEnabled = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										enabled: !!root.settings.soundEnabled
										Text { text: "Sound Volume:"; color: "#565f89"; Layout.minimumWidth: 130 }
										Text { text: "0%"; color: "#565f89"; font.pixelSize: 10 }
										Slider {
											Layout.fillWidth: true
											from: 0; to: 1
											value: root.settings.soundVolume !== undefined ? root.settings.soundVolume : 0.9
											onMoved: root.settings.soundVolume = value
											background: Rectangle { height: 4; color: "#414868"; radius: 2
												Rectangle { height: parent.height; width: parent.width * parent.parent.position; color: "#7aa2f7"; radius: 2 }
											}
											handle: Rectangle {
												x: parent.leftPadding + parent.availableWidth * parent.visualPosition - width/2
												y: parent.topPadding + parent.availableHeight/2 - height/2
												width: 14; height: 14; radius: 7; color: "#7da6ff"; border.color: "#414868"; border.width: 1
											}
										}
										Text { text: "100%"; color: "#565f89"; font.pixelSize: 10 }
									}

									RowLayout {
										spacing: 12
										enabled: !!root.settings.soundEnabled
										Text { text: "Music Volume:"; color: "#565f89"; Layout.minimumWidth: 130 }
										Text { text: "0%"; color: "#565f89"; font.pixelSize: 10 }
										Slider {
											Layout.fillWidth: true
											from: 0; to: 1
											value: root.settings.musicVolume !== undefined ? root.settings.musicVolume : 0.53
											onMoved: root.settings.musicVolume = value
											background: Rectangle { height: 4; color: "#414868"; radius: 2
												Rectangle { height: parent.height; width: parent.width * parent.parent.position; color: "#7aa2f7"; radius: 2 }
											}
											handle: Rectangle {
												x: parent.leftPadding + parent.availableWidth * parent.visualPosition - width/2
												y: parent.topPadding + parent.availableHeight/2 - height/2
												width: 14; height: 14; radius: 7; color: "#7da6ff"; border.color: "#414868"; border.width: 1
											}
										}
										Text { text: "100%"; color: "#565f89"; font.pixelSize: 10 }
									}

									Item { Layout.fillHeight: true }
								}
							}

							// --- Gameplay ---
							ScrollView {
								contentWidth: availableWidth
								ColumnLayout {
									width: parent.width
									spacing: 14

									RowLayout {
										spacing: 12
										Text { text: "Mouse Sensitivity:"; color: "#565f89"; Layout.minimumWidth: 150 }
										Text { text: "0.2"; color: "#565f89"; font.pixelSize: 10 }
										Slider {
											Layout.fillWidth: true
											from: 0.2; to: 10.0
											value: root.settings.mouseSensitivity !== undefined ? root.settings.mouseSensitivity : 3.0
											onMoved: root.settings.mouseSensitivity = value
											background: Rectangle { height: 4; color: "#414868"; radius: 2
												Rectangle { height: parent.height; width: parent.width * parent.parent.position; color: "#7aa2f7"; radius: 2 }
											}
											handle: Rectangle {
												x: parent.leftPadding + parent.availableWidth * parent.visualPosition - width/2
												y: parent.topPadding + parent.availableHeight/2 - height/2
												width: 14; height: 14; radius: 7; color: "#7da6ff"; border.color: "#414868"; border.width: 1
											}
										}
										Text { text: "10.0"; color: "#565f89"; font.pixelSize: 10 }
									}

									RowLayout {
										spacing: 12
										Text { text: "Invert Mouse:"; color: "#565f89"; Layout.minimumWidth: 150 }
										CheckBox {
											checked: !!root.settings.invertMouse
											onToggled: root.settings.invertMouse = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Enable Joystick:"; color: "#565f89"; Layout.minimumWidth: 150 }
										CheckBox {
											checked: !!root.settings.joystickEnabled
											onToggled: root.settings.joystickEnabled = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Auto-Center Camera:"; color: "#565f89"; Layout.minimumWidth: 150 }
										CheckBox {
											checked: !!root.settings.autoCenterCamera
											onToggled: root.settings.autoCenterCamera = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Move While Casting:"; color: "#565f89"; Layout.minimumWidth: 150 }
										CheckBox {
											checked: !!root.settings.moveWhileCasting
											onToggled: root.settings.moveWhileCasting = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Auto-Quaff Potions:"; color: "#565f89"; Layout.minimumWidth: 150 }
										CheckBox {
											checked: !!root.settings.autoQuaff
											onToggled: root.settings.autoQuaff = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Screen Flashes:"; color: "#565f89"; Layout.minimumWidth: 150 }
										CheckBox {
											checked: !!root.settings.screenFlashes
											onToggled: root.settings.screenFlashes = checked
											indicator: Rectangle {
												width: 18; height: 18; radius: 3
												color: parent.checked ? "#7aa2f7" : "#13141c"
												border.color: "#414868"; border.width: 1
												Text { anchors.centerIn: parent; text: "\u2713"; color: "#c0caf5"; visible: parent.parent.checked }
											}
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Difficulty:"; color: "#565f89"; Layout.minimumWidth: 150 }
										ComboBox {
											Layout.fillWidth: true
											model: ["Easy", "Medium", "Hard"]
											currentIndex: ["easy", "medium", "hard"].indexOf(root.settings.difficulty)
											onActivated: (idx) => root.settings.difficulty = ["easy", "medium", "hard"][idx]
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									RowLayout {
										spacing: 12
										Text { text: "Control Mode:"; color: "#565f89"; Layout.minimumWidth: 150 }
										ComboBox {
											Layout.fillWidth: true
											model: ["Classic", "Modern"]
											currentIndex: root.settings.controlMode === "modern" ? 1 : 0
											onActivated: (idx) => root.settings.controlMode = idx === 1 ? "modern" : "classic"
											background: Rectangle { color: "#13141c"; border.color: "#414868"; border.width: 1; radius: 4 }
											contentItem: Text { text: parent.displayText; color: "#a9b1d6"; leftPadding: 8 }
										}
									}

									Item { Layout.fillHeight: true }
								}
							}
						}
					}
				}
			}

			RowLayout {
				Layout.fillWidth: true
				height: 48
				spacing: 12

				Button {
					Layout.fillWidth: true
					text: "New Game"
					enabled: !root.isProcessing && !!root.req
					background: Rectangle {
						color: parent.enabled ? "#24283b" : "#13141c"
						radius: 6
						border.color: parent.enabled ? "#9ece6a" : "#414868"
						border.width: 1
					}
					contentItem: Text {
						text: parent.text
						color: parent.enabled ? "#c0caf5" : "#565f89"
						font.bold: true
						font.pixelSize: 13
						horizontalAlignment: Text.AlignHCenter
					}
					onClicked: root.commitAndExit("newGame")
				}

				Button {
					Layout.fillWidth: true
					text: "Continue"
					enabled: !root.isProcessing && !!root.req && root.selectedSaveIndex >= 0
					background: Rectangle {
						color: parent.enabled ? "#24283b" : "#13141c"
						radius: 6
						border.color: parent.enabled ? "#7aa2f7" : "#414868"
						border.width: 1
					}
					contentItem: Text {
						text: parent.text
						color: parent.enabled ? "#c0caf5" : "#565f89"
						font.bold: true
						font.pixelSize: 13
						horizontalAlignment: Text.AlignHCenter
					}
					onClicked: {
						const save = root.req.saves[root.selectedSaveIndex]
						root.commitAndExit("continue", {
							saveIndex: save.saveIndex,
							slot: save.slot,
							usesSlotDirectory: save.usesSlotDirectory ? "true" : "false",
							savePath: save.savePath
						})
					}
				}

				Button {
					Layout.fillWidth: true
					text: "Quit"
					enabled: !root.isProcessing
					background: Rectangle {
						color: parent.enabled ? "#24283b" : "#13141c"
						radius: 6
						border.color: parent.enabled ? "#f7768e" : "#414868"
						border.width: 1
					}
					contentItem: Text {
						text: parent.text
						color: parent.enabled ? "#c0caf5" : "#565f89"
						font.bold: true
						font.pixelSize: 13
						horizontalAlignment: Text.AlignHCenter
					}
					onClicked: root.commitAndExit("quit")
				}
			}
		}
	}
}
