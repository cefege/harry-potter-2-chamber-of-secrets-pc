//===============================================================================
// #81 Quong Po
//===============================================================================

class  WCPo extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardPoTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Posmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPoBigTexture FILE=TEXTURES\menu\Folio\Cards\Pobig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Quong Po";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=81
    skin(0)=Texture'HProps.skins.WizardCardPoTex0'
	textureBig=Texture'WizCardPoBigTexture'
	strDescriptionId="WizCard_0075"
}
