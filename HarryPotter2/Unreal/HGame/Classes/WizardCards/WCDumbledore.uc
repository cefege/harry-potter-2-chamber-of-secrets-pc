//===============================================================================
//  #101 Albus Dumbledore Headmaster Hogwarts
//===============================================================================

class  WCDumbledore extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardDumbledoreTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\dumbledoresmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardDumbledoreBigTexture FILE=TEXTURES\menu\Folio\Cards\Dumbledorebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardAlbusBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\101_Albus_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardAlbusBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\101_Albus_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardAlbusBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\101_Albus_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Albus Dumbledore";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=101
    skin(0)=Texture'HProps.skins.WizardCardDumbledoreTex0'
	textureBig=Texture'WizCardDumbledoreBigTexture'
	strDescriptionId="WizCard_0100"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardAlbusBigTextureLayer0';
	textureLayers(1)=Texture'WizCardAlbusBigTextureLayer1';
	textureLayers(2)=Texture'WizCardAlbusBigTextureLayer2';

}


