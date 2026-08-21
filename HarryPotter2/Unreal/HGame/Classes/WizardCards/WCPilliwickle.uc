//===============================================================================
// #67 Justus Pilliwickle
//===============================================================================

class  WCPilliwickle extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardPilliwickleTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Pilliwicklesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPilliwickleBigTexture FILE=TEXTURES\menu\Folio\Cards\Pilliwicklebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Justus Pilliwickle";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=67
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE120"
    skin(0)=Texture'HProps.skins.WizardCardPilliwickleTex0'
	textureBig=Texture'WizCardPilliwickleBigTexture'
	strDescriptionId="WizCard_0061"
}
