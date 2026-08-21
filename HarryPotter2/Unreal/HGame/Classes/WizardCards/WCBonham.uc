//===============================================================================
// #75 Mungo Bonham
//===============================================================================

class  WCBonham extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardBonhamTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Bonhamsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBonhamBigTexture FILE=TEXTURES\menu\Folio\Cards\Bonhambig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Mungo Bonham";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=75
    skin(0)=Texture'HProps.skins.WizardCardBonhamTex0'
	textureBig=Texture'WizCardBonhamBigTexture'
	strDescriptionId="WizCard_0058"
}
