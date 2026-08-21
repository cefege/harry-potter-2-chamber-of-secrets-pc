//===============================================================================
// #40	Carlotta Pinkstone
//===============================================================================

class  WCPinkstone extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardPinkstoneTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Pinkstonesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPinkstoneBigTexture FILE=TEXTURES\menu\Folio\Cards\Pinkstonebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardCarlottaBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\40_Carlotta_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardCarlottaBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\40_Carlotta_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardCarlottaBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\40_Carlotta_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Carlotta Pinkstone";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=40
    skin(0)=Texture'HProps.skins.WizardCardPinkstoneTex0'
	textureBig=Texture'WizCardPinkstoneBigTexture'
	strDescriptionId="WizCard_0017"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardCarlottaBigTextureLayer0';
	textureLayers(1)=Texture'WizCardCarlottaBigTextureLayer1';
	textureLayers(2)=Texture'WizCardCarlottaBigTextureLayer2';

}
