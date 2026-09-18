# Resident Evil 9 Save File Editor

## Goal

Provide an easy way to decrypt, edit and re encrypt a RE9 Save file.

Endview would be to have a UI, where you input a save file and and Optional Steam Id
The Tool will then:
- Get the SteamId from the file, if not passed as input.
- Decrypt the saved file
- Display the Inventory slots of the Saved File.
- Display other Save information (nicely displayed, not editable for now)

Then the UI will allow to:
- Modify the Inventory slot quantities
- Export the new modified saved file to be used in the game.


## Sort term Plan

[ ] Via CLI, allow to : decrypt / encrypt a saved file, given as parameter (inputfile.bin), then save it into a new file (inputfile_new.bin) 
[ ] Provide a UI that will allow to do the same, asking for file, steam id, clicking on "Edit" or something, will decrypt the dsave file, display basic info, then a button "Export Save File" will reencrypt the file
[ ] Via UI, be able to display, for a given save file, the list of the items in the inventory, with their quantity
